use crate::error::{AnonymizerError, Result};
use crate::types::{ProcessControl, ProcessProgress};

pub(crate) fn check_canceled(control: &mut Option<&mut ProcessControl<'_>>) -> Result<()> {
    let Some(control) = control.as_deref_mut() else {
        return Ok(());
    };
    let Some(should_cancel) = control.should_cancel else {
        return Ok(());
    };
    if should_cancel() {
        Err(AnonymizerError::Canceled)
    } else {
        Ok(())
    }
}

pub(crate) fn report_progress(
    control: &mut Option<&mut ProcessControl<'_>>,
    rows_processed: usize,
) {
    let Some(control) = control.as_deref_mut() else {
        return;
    };
    let Some(on_progress) = control.on_progress.as_deref_mut() else {
        return;
    };
    on_progress(ProcessProgress { rows_processed });
}
