//! What the conversation agent runs as: the saved agent type `default`
//! (`CONVERSATION_AGENT_TYPE`). Its model, permission mode, tools and personality prompt
//! replace the built-in ones; anything missing or unusable keeps the built-in value, and a
//! type that is missing, unreadable or not a claude type keeps all of them.

use crate::definitions::{AdapterKind, AgentDef};
use crate::model::{PermissionMode, TownhallModel};

use super::args::{allowed_tools, DENIED_TOOLS};
use super::prompt::INSTRUCTIONS;

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
    fn built_in(setting_model: TownhallModel) -> Self {
        Self {
            model: setting_model.cli_value().into(),
            permission_mode: PermissionMode::Default,
            tools: allowed_tools(),
            prompt: INSTRUCTIONS.into(),
        }
    }

    /// `def` applied on the built-in values. Its tools lose every entry that writes or
    /// runs things, so a saved type cannot let the session edit a repo.
    pub fn from_type(def: Option<&AgentDef>, setting_model: TownhallModel) -> Self {
        let built_in = Self::built_in(setting_model);
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

    fn built_in(model: TownhallModel) -> Persona {
        Persona {
            model: model.cli_value().into(),
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
        let got = Persona::from_type(None, TownhallModel::Opus);
        assert_eq!(got, built_in(TownhallModel::Opus));
        assert_eq!(got.permission_mode, PermissionMode::Default);
        assert_eq!(got.model, "opus");
    }

    #[test]
    fn a_type_sets_model_tools_and_prompt() {
        let d = def(
            "model = \"opus\"\ntools = [\"Read\", \"mcp__townhall__create_task\"]\nprompt = \"be kind\"\n",
        );
        let got = Persona::from_type(Some(&d), TownhallModel::Sonnet);
        assert_eq!(got.model, "opus");
        assert_eq!(got.tools, ["Read", "mcp__townhall__create_task"]);
        assert_eq!(got.prompt, "be kind");
    }

    #[test]
    fn blank_fields_keep_the_built_in_values() {
        let d = def("model = \" \"\nprompt = \"  \"\n");
        let got = Persona::from_type(Some(&d), TownhallModel::Haiku);
        assert_eq!(got, built_in(TownhallModel::Haiku));
    }

    #[test]
    fn tools_that_write_or_run_things_are_never_allowed() {
        let d = def("tools = [\"Read\", \"Bash(git *)\", \"Edit\", \"Write\", \"Grep\"]\n");
        let got = Persona::from_type(Some(&d), TownhallModel::Sonnet);
        assert_eq!(got.tools, ["Read", "Grep"]);
        let only_denied = def("tools = [\"Bash\"]\n");
        let got = Persona::from_type(Some(&only_denied), TownhallModel::Sonnet);
        assert_eq!(got.tools, allowed_tools());
    }

    #[test]
    fn the_conversation_runs_in_its_types_mode_and_never_skips_permissions() {
        let mode = |own: &str| {
            let d = def(&format!("permission_mode = \"{own}\"\n"));
            Persona::from_type(Some(&d), TownhallModel::Sonnet).permission_mode
        };
        assert_eq!(mode("acceptEdits"), PermissionMode::AcceptEdits);
        assert_eq!(mode("default"), PermissionMode::Default);
        assert_eq!(mode("plan"), PermissionMode::Plan);
        assert_eq!(mode("bypassPermissions"), PermissionMode::Default);
        let unset = Persona::from_type(Some(&def("")), TownhallModel::Sonnet);
        assert_eq!(unset.permission_mode, PermissionMode::Default);
    }

    #[test]
    fn a_shell_type_keeps_the_built_in_behaviour() {
        let shell = parse_agent(
            "name = \"default\"\nadapter = \"shell\"\ncommand = \"ls\"\nmodel = \"opus\"\n",
        )
        .unwrap()
        .def;
        let got = Persona::from_type(Some(&shell), TownhallModel::Sonnet);
        assert_eq!(got, built_in(TownhallModel::Sonnet));
    }
}
