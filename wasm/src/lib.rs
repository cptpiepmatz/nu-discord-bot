use std::sync::Arc;

use exports::nu::discord_bot::nu::{Guest, GuestExecutor};
use nu_protocol::{
    Config, PipelineData, Span, Value,
    debugger::WithoutDebug,
    engine::{self, EngineState, Stack, StateWorkingSet},
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

type File = exports::nu::discord_bot::nu::File;

impl GuestExecutor for Executor {
    fn new() -> Self {
        Self {
            engine_state: initial_engine_state(),
            stack: Stack::default(),
        }
    }

    fn execute(&self, fname: String, source: String, file: Option<File>) -> String {
        let source = format!("{source} | table --expand");
        let source = source.as_bytes();
        let mut engine_state = self.engine_state.clone();
        let mut stack = self.stack.clone();
        let mut working_set = StateWorkingSet::new(&engine_state);
        let block = nu_parser::parse(&mut working_set, Some(&fname), source, false);

        if let Some(error) = working_set.parse_errors.into_iter().next() {
            return "some parse error".into();
        }

        if let Some(error) = working_set.compile_errors.into_iter().next() {
            return "some compile error".into();
        }

        let input = match file {
            Some(File::Text(text)) => Value::string(text, Span::unknown()),
            Some(File::Bytes(bytes)) => Value::binary(bytes, Span::unknown()),
            None => Value::nothing(Span::unknown()),
        };
        let input = PipelineData::Value(input, None);

        engine_state.merge_delta(working_set.delta).unwrap();
        let res = nu_engine::eval_block::<WithoutDebug>(&engine_state, &mut stack, &block, input);
        let res = res.unwrap();

        let output = res.into_value(Span::unknown()).unwrap();
        let output = output.into_string().unwrap();
        output
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
