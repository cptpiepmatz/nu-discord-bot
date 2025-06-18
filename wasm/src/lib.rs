use std::sync::Arc;

use exports::nu::discord_bot::nu::{ExecuteError, ExecuteOk, File, Guest, GuestExecutor};
use nu_protocol::{
    Config, PipelineData, Span, Value,
    debugger::WithoutDebug,
    engine::{EngineState, Stack, StateWorkingSet},
};

wit_bindgen::generate!(in "../wit");

struct NuComponent;

impl Guest for NuComponent {
    type Executor = Executor;
}

struct Executor {
    engine_state: EngineState,
    stack: Stack,
}

impl GuestExecutor for Executor {
    fn new() -> Self {
        Self {
            engine_state: initial_engine_state(),
            stack: Stack::default(),
        }
    }

    fn execute(
        &self,
        fname: String,
        source: String,
        file: Option<File>,
        cols: u16,
    ) -> Result<ExecuteOk, ExecuteError> {
        let source = format!("{source} | table --expand --width {cols} | into string");
        let source = source.as_bytes();
        let mut engine_state = self.engine_state.clone();
        let mut stack = self.stack.clone();
        let mut working_set = StateWorkingSet::new(&engine_state);
        let block = nu_parser::parse(&mut working_set, Some(&fname), source, false);

        if let Some(error) = working_set.parse_errors.iter().next() {
            return Ok(ExecuteOk::Error(nu_protocol::format_cli_error(
                &working_set,
                error,
            )));
        }

        if let Some(error) = working_set.compile_errors.iter().next() {
            return Ok(ExecuteOk::Error(nu_protocol::format_cli_error(
                &working_set,
                error,
            )));
        }

        let input = match file {
            Some(File::Text(text)) => Value::string(text, Span::unknown()),
            Some(File::Bytes(bytes)) => Value::binary(bytes, Span::unknown()),
            None => Value::nothing(Span::unknown()),
        };
        let input = PipelineData::Value(input, None);

        engine_state
            .merge_delta(working_set.delta)
            .map_err(|_| ExecuteError::MergeDelta)?;
        let res = nu_engine::eval_block::<WithoutDebug>(&engine_state, &mut stack, &block, input);
        let res = match res {
            Err(err) => return Ok(ExecuteOk::Error(format!("{err:#?}"))),
            Ok(res) => res,
        };

        let output = res
            .into_value(Span::unknown())
            .map_err(|_| ExecuteError::IntoValue)?;
        let output = output.into_string().map_err(|_| ExecuteError::IntoString)?;
        Ok(ExecuteOk::Value(output))
    }
}

export!(NuComponent);

fn initial_engine_state() -> EngineState {
    let engine_state = nu_cmd_lang::create_default_context();
    let engine_state = nu_command::add_shell_command_context(engine_state);
    let engine_state = nu_cmd_extra::add_extra_command_context(engine_state);

    let engine_state = configure_engine_state(engine_state);

    engine_state
}

fn configure_engine_state(mut engine_state: EngineState) -> EngineState {
    engine_state.history_enabled = false;
    engine_state.is_interactive = false;
    engine_state.is_login = false;

    engine_state.config = Arc::new(Config {
        use_ansi_coloring: true.into(),
        ..Default::default()
    });

    engine_state
}
