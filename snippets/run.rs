use crate::{
    error::Result,
    stage,
    utility::{Phase, Terminal},
};

pub fn run(terminal: &mut Terminal) -> Result<()> {
    stage![terminal, |frame| {}, &mut |_, key_code| match key_code {
        _ => Ok(Phase::Return(())),
    }];
}
