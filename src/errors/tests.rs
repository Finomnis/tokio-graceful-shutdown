use crate::BoxedError;

use super::*;

fn examine_report(report: miette::Report) {
    println!("{}", report);
    println!("{:?}", report);
    // Convert to std::error::Error
    let boxed_error: BoxedError = report.into();
    println!("{}", boxed_error);
    println!("{:?}", boxed_error);
}

#[test]
fn errors_can_be_converted_to_diagnostic() {
    examine_report(GracefulShutdownError::ShutdownTimeout::<BoxedError>(Box::new([])).into());
    examine_report(GracefulShutdownError::SubsystemsFailed::<BoxedError>(Box::new([])).into());
    examine_report(SubsystemJoinError::SubsystemsFailed::<BoxedError>(Arc::new([])).into());
    examine_report(SubsystemError::Panicked::<BoxedError>("".into()).into());
    examine_report(
        SubsystemError::Failed::<BoxedError>("".into(), SubsystemFailure("".into())).into(),
    );
    examine_report(CancelledByShutdown.into());
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
