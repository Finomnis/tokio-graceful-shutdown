use tracing_test::traced_test;

use crate::BoxedError;

use super::*;

fn examine_report(
    error: impl miette::Diagnostic + Sync + Send + 'static,
) {
    println!("{}", error);
    println!("{:?}", error);
    println!("{:?}", error.source());
    println!("{}", error.code().unwrap());
    // Convert to report
    let report: miette::Report = error.into();
    println!("{}", report);
    println!("{:?}", report);
    // Convert to std::error::Error
    let boxed_error: BoxedError = report.into();
    println!("{}", boxed_error);
    println!("{:?}", boxed_error);
}

#[test]
fn errors_can_be_converted_to_diagnostic() {
    examine_report(GracefulShutdownError::ShutdownTimeout::<BoxedError>(
        Box::new([]),
    ));
    examine_report(GracefulShutdownError::SubsystemsFailed::<BoxedError>(
        Box::new([]),
    ));
    examine_report(SubsystemJoinError::SubsystemsFailed::<BoxedError>(
        Arc::new([]),
    ));
    examine_report(SubsystemError::Panicked::<BoxedError>("".into()));
    examine_report(SubsystemError::Failed::<BoxedError>(
        "".into(),
        SubsystemFailure("".into()),
    ));
    examine_report(CancelledByShutdown);
}

#[test]
fn extract_related_from_graceful_shutdown_error() {
    let related = || {
        Box::new([
            SubsystemError::Failed("a".into(), SubsystemFailure(String::from("A").into())),
            SubsystemError::Panicked("b".into()),
        ])
    };

    let matches_related = |data: &[SubsystemError<BoxedError>]| {
        let mut iter = data.iter();

        let elem = iter.next().unwrap();
        assert_eq!(elem.name(), "a");
        assert!(matches!(elem, SubsystemError::Failed(_, _)));

        let elem = iter.next().unwrap();
        assert_eq!(elem.name(), "b");
        assert!(matches!(elem, SubsystemError::Panicked(_)));

        assert!(iter.next().is_none());
    };

    matches_related(GracefulShutdownError::ShutdownTimeout(related()).get_subsystem_errors());
    matches_related(GracefulShutdownError::SubsystemsFailed(related()).get_subsystem_errors());
    matches_related(&GracefulShutdownError::ShutdownTimeout(related()).into_subsystem_errors());
    matches_related(&GracefulShutdownError::SubsystemsFailed(related()).into_subsystem_errors());
}

#[test]
fn extract_contained_error_from_convert_subsystem_failure() {
    let msg = "MyFailure".to_string();
    let failure = SubsystemFailure(msg.clone());

    assert_eq!(&msg, failure.get_error());
    assert_eq!(msg, *failure);
    assert_eq!(msg, failure.into_error());
}

#[test]
#[traced_test]
fn handle_dropped_errors() {
    handle_dropped_error(Err(mpsc::error::SendError(BoxedError::from(String::from(
        "ABC",
    )))));

    assert!(logs_contain("An error got dropped: \"ABC\""));
}

#[test]
#[traced_test]
fn handle_unhandled_stopreasons() {
    handle_unhandled_stopreason(Some(SubsystemError::<BoxedError>::Panicked(Arc::from(
        "def",
    ))));

    assert!(logs_contain("Unhandled stop reason: Panicked(\"def\")"));
}
