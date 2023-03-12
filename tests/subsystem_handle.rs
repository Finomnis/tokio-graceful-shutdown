use tokio_graceful_shutdown::*;

/// **This should show:**
///
/// Time to shut down!
/// Shutting down thing task 1
/// Shutting down thing task 0
/// Shut down task 1
/// Shut down task 0
/// All thing tasks have shut down, propagating up
/// Stopping monitoring
#[tokio::test]
async fn subsystem_handle_usecase() {
    let toplevel = Toplevel::<anyhow::Error>::new();

    // Obviously these two things are more complex in reality
    // thing needs &mut monitoring to initialize, and then that needs to be moved
    // to the Monitoring task
    let mut monitoring_registry = ();
    fn init_thing(_monitoring_registry: &mut ()) {}

    let tasks = Toplevel::<anyhow::Error>::nested(toplevel.subsystem_handle(), "Tasks");
    for task in 0..2 {
        let thing_to_work_on = init_thing(&mut monitoring_registry);
        // By the way I would also like to not reallocate that string, although it does not matter
        // too much
        tasks
            .subsystem_handle()
            .start(&format!("Task {task}"), move |subsystem| async move {
                // This simulates normal work with, but obviously it would normally wait on select
                // on `thing` and on_shutdown_requested
                let _thing = thing_to_work_on;
                subsystem.on_shutdown_requested().await;

                // Simulates the time it takes to gracefully shutdown
                println!("Shutting down thing task {task}");
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                println!("Shut down task {task}");
                Ok::<(), anyhow::Error>(())
            });
    }
    // simulate ctrl c
    let tasks_ss = tasks.subsystem_handle().clone();
    tokio::task::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        println!("Time to shut down!");
        tasks_ss.request_shutdown();
    });
    toplevel
        .start("Tasks", |ss| async move {
            let res = tasks
                //.catch_signals() - normally we would put that here
                .handle_shutdown_requests(std::time::Duration::from_millis(30))
                .await;
            println!("All thing tasks have shut down, propagating up");
            ss.request_global_shutdown();
            res
        })
        .start("Monitoring", move |ss| async move {
            let _monitoring = monitoring_registry;
            // imitates how quickly monitoring stops
            ss.on_shutdown_requested().await;
            println!("Stopping monitoring");
            Ok::<_, anyhow::Error>(())
        })
        .handle_shutdown_requests(std::time::Duration::from_millis(10))
        .await
        .unwrap();
}
