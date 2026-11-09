//! What the conversation agent runs as: the saved agent type `default`
//! (`CONVERSATION_AGENT_TYPE`). Its model, permission mode, tools and personality prompt
//! replace the built-in ones; anything missing or unusable keeps the built-in value, and a
//! type that is missing, unreadable or not a claude type keeps all of them.

use crate::definitions::{AdapterKind, AgentDef};
use crate::model::PermissionMode;

use super::args::{allowed_tools, DENIED_TOOLS};
use super::prompt::INSTRUCTIONS;

pub const FALLBACK_MODEL: &str = "opus";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Persona {
    /// The `--model` value.
    pub model: String,
    pub permission_mode: PermissionMode,
    /// Every tool the session may use without asking, built-in and MCP.
    pub tools: Vec<String>,
    /// The personality and rules, before the repo notes and the profile.
    pub prompt: String,
}

impl Persona {
    /// The built-in behaviour: `def` is `None`, or a type the conversation cannot run as.
    fn built_in() -> Self {
        Self {
            model: FALLBACK_MODEL.into(),
            permission_mode: PermissionMode::Default,
            tools: allowed_tools(),
            prompt: INSTRUCTIONS.into(),
        }
    }

    /// `def` applied on the built-in values. Its tools lose every entry that writes or
    /// runs things, so a saved type cannot let the session edit a repo.
    pub fn from_type(def: Option<&AgentDef>) -> Self {
        let built_in = Self::built_in();
        let Some(def) = def.filter(|d| d.adapter == AdapterKind::Claude) else {
            return built_in;
        };
        let fields = def.launch_fields();
        let tools: Vec<String> = fields
            .allowed_tools
            .into_iter()
            .filter(|tool| !DENIED_TOOLS.contains(&tool.split('(').next().unwrap_or(tool)))
            .collect();
        Self {
            model: fields.model.unwrap_or(built_in.model),
            permission_mode: def.conversation_mode(),
            tools: if tools.is_empty() {
                built_in.tools
            } else {
                tools
            },
            prompt: fields.system_prompt.unwrap_or(built_in.prompt),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definitions::parse_agent;

    fn def(extra: &str) -> AgentDef {
        let source = format!("name = \"default\"\nadapter = \"claude\"\n{extra}");
        parse_agent(&source).unwrap().def
    }

    fn built_in() -> Persona {
        Persona {
            model: FALLBACK_MODEL.into(),
            permission_mode: PermissionMode::Default,
            tools: allowed_tools(),
            prompt: INSTRUCTIONS.into(),
        }
    }

    #[test]
    fn the_seeded_prompt_is_the_built_in_one() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::store::migrate::apply(&mut conn, &crate::store::migrate::all()).unwrap();
        let source: String = conn
            .query_row(
                "select source from agents where name = 'default'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            source.contains(INSTRUCTIONS),
            "keep the seed and INSTRUCTIONS in step"
        );
        let def = parse_agent(&source).unwrap().def;
        assert_eq!(Persona::from_type(Some(&def)).tools, allowed_tools());
    }

    #[test]
    fn no_type_keeps_the_built_in_behaviour() {
        let got = Persona::from_type(None);
        assert_eq!(got, built_in());
        assert_eq!(got.permission_mode, PermissionMode::Default);
        assert_eq!(got.model, "opus");
    }

    #[test]
    fn a_type_sets_model_tools_and_prompt() {
        let d = def(
            "model = \"haiku\"\ntools = [\"Read\", \"mcp__townhall__create_task\"]\nprompt = \"be kind\"\n",
        );
        let got = Persona::from_type(Some(&d));
        assert_eq!(got.model, "haiku");
        assert_eq!(got.tools, ["Read", "mcp__townhall__create_task"]);
        assert_eq!(got.prompt, "be kind");
    }

    #[test]
    fn blank_fields_keep_the_built_in_values() {
        let d = def("model = \" \"\nprompt = \"  \"\n");
        let got = Persona::from_type(Some(&d));
        assert_eq!(got, built_in());
        assert_eq!(got.model, "opus");
    }

    #[test]
    fn tools_that_write_or_run_things_are_never_allowed() {
        let d = def("tools = [\"Read\", \"Bash(git *)\", \"Edit\", \"Write\", \"Grep\"]\n");
        let got = Persona::from_type(Some(&d));
        assert_eq!(got.tools, ["Read", "Grep"]);
        let only_denied = def("tools = [\"Bash\"]\n");
        let got = Persona::from_type(Some(&only_denied));
        assert_eq!(got.tools, allowed_tools());
    }

    #[test]
    fn the_conversation_runs_in_its_types_mode_and_never_skips_permissions() {
        let mode = |own: &str| {
            let d = def(&format!("permission_mode = \"{own}\"\n"));
            Persona::from_type(Some(&d)).permission_mode
        };
        assert_eq!(mode("acceptEdits"), PermissionMode::AcceptEdits);
        assert_eq!(mode("default"), PermissionMode::Default);
        assert_eq!(mode("plan"), PermissionMode::Plan);
        assert_eq!(mode("bypassPermissions"), PermissionMode::Default);
        let unset = Persona::from_type(Some(&def("")));
        assert_eq!(unset.permission_mode, PermissionMode::Default);
    }

    #[test]
    fn a_shell_type_keeps_the_built_in_behaviour() {
        let shell = parse_agent(
            "name = \"default\"\nadapter = \"shell\"\ncommand = \"ls\"\nmodel = \"opus\"\n",
        )
        .unwrap()
        .def;
        let got = Persona::from_type(Some(&shell));
        assert_eq!(got, built_in());
    }
}
