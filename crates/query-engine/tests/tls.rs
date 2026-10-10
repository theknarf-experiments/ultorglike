#[test]
#[ignore = "requires TLS fixture; scripts/check-tls.sh runs this test"]
fn verifies_server_certificate_and_hostname() {
    let url = std::env::var("TEST_TLS_DATABASE_URL").unwrap();
    let mut client = query_engine::executor::connect(&url).expect("trusted localhost certificate");
    let tls: bool = client
        .query_one(
            "SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()",
            &[],
        )
        .unwrap()
        .get(0);
    assert!(tls);
    let wrong_host = url.replace("@localhost:", "@127.0.0.1:");
    assert!(
        query_engine::executor::connect(&wrong_host).is_err(),
        "Hostname mismatch must fail"
    );
    // This fixture test runs alone in its process, with no concurrent env readers.
    let cert = std::env::var_os("PGSSLROOTCERT").unwrap();
    std::env::remove_var("PGSSLROOTCERT");
    let untrusted = query_engine::executor::connect(&url);
    std::env::set_var("PGSSLROOTCERT", cert);
    assert!(
        untrusted.is_err(),
        "Untrusted self-signed certificate must fail"
    );
}
