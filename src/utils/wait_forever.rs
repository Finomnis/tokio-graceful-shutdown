pub async fn wait_forever() -> ! {
    loop {
        std::future::pending::<()>().await;
    }
}
