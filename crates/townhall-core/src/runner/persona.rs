//! What the conversation agent runs as: the saved agent type `default`
//! (`CONVERSATION_AGENT_TYPE`). Its model, permission mode, tools and personality prompt
//! replace the built-in ones; anything missing or unusable keeps the built-in value, and a
//! type that is missing, unreadable or not a claude type keeps all of them.

use crate::definitions::{AdapterKind, AgentDef};
use crate::model::PermissionMode;

use super::args::{allowed_tools, DENIED_TOOLS};
use super::prompt::INSTRUCTIONS;

/// The model a turn runs on when the type `default` sets none (or cannot be used).
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
    pub fn from_type(def: Option<&AgentDef>, setting_mode: PermissionMode) -> Self {
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
            permission_mode: def.conversation_mode(setting_mode),
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
        let sql = include_str!("../store/migrations/0806_seed_default.sql").replace("''", "'");
        assert!(
            sql.contains(INSTRUCTIONS),
            "keep the seed and INSTRUCTIONS in step"
        );
    }

    #[test]
    fn no_type_keeps_the_built_in_behaviour() {
        let got = Persona::from_type(None, PermissionMode::AcceptEdits);
        assert_eq!(got, built_in());
        assert_eq!(got.permission_mode, PermissionMode::Default);
        assert_eq!(got.model, "opus");
    }

    #[test]
    fn a_type_sets_model_tools_and_prompt() {
        let d = def(
            "model = \"haiku\"\ntools = [\"Read\", \"mcp__townhall__create_task\"]\nprompt = \"be kind\"\n",
        );
        let got = Persona::from_type(Some(&d), PermissionMode::AcceptEdits);
        assert_eq!(got.model, "haiku");
        assert_eq!(got.tools, ["Read", "mcp__townhall__create_task"]);
        assert_eq!(got.prompt, "be kind");
    }

    #[test]
    fn blank_fields_keep_the_built_in_values() {
        let d = def("model = \" \"\nprompt = \"  \"\n");
        let got = Persona::from_type(Some(&d), PermissionMode::AcceptEdits);
        assert_eq!(got, built_in());
        assert_eq!(got.model, "opus");
    }

    #[test]
    fn tools_that_write_or_run_things_are_never_allowed() {
        let d = def("tools = [\"Read\", \"Bash(git *)\", \"Edit\", \"Write\", \"Grep\"]\n");
        let got = Persona::from_type(Some(&d), PermissionMode::AcceptEdits);
        assert_eq!(got.tools, ["Read", "Grep"]);
        let only_denied = def("tools = [\"Bash\"]\n");
        let got = Persona::from_type(Some(&only_denied), PermissionMode::AcceptEdits);
        assert_eq!(got.tools, allowed_tools());
    }

    #[test]
    fn the_permission_mode_is_never_looser_than_the_setting() {
        let mode = |own: &str, setting| {
            let d = def(&format!("permission_mode = \"{own}\"\n"));
            Persona::from_type(Some(&d), setting).permission_mode
        };
        assert_eq!(
            mode("acceptEdits", PermissionMode::AcceptEdits),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            mode("acceptEdits", PermissionMode::Default),
            PermissionMode::Default
        );
        assert_eq!(
            mode("acceptEdits", PermissionMode::Plan),
            PermissionMode::Plan
        );
        assert_eq!(
            mode("default", PermissionMode::BypassPermissions),
            PermissionMode::Default
        );
        assert_eq!(
            mode("plan", PermissionMode::AcceptEdits),
            PermissionMode::Plan
        );
        let unset = Persona::from_type(Some(&def("")), PermissionMode::BypassPermissions);
        assert_eq!(unset.permission_mode, PermissionMode::Default);
    }

    #[test]
    fn a_shell_type_keeps_the_built_in_behaviour() {
        let shell = parse_agent(
            "name = \"default\"\nadapter = \"shell\"\ncommand = \"ls\"\nmodel = \"opus\"\n",
        )
        .unwrap()
        .def;
        let got = Persona::from_type(Some(&shell), PermissionMode::AcceptEdits);
        assert_eq!(got, built_in());
    }
}
