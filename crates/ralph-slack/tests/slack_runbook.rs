const SLACK_LIVE_SMOKE_RUNBOOK: &str =
    include_str!("../../../docs/runbooks/slack-live-smoke-test.md");

#[test]
fn live_smoke_runbook_checks_loop_state_and_logs() {
    assert!(
        SLACK_LIVE_SMOKE_RUNBOOK.contains(".ralph/slack-state.json"),
        "live smoke runbook should tell operators to verify Slack state"
    );
    assert!(
        SLACK_LIVE_SMOKE_RUNBOOK.contains(".ralph/slack-loop-logs/<loop-id>.log"),
        "live smoke runbook should tell operators to verify the per-loop log"
    );
}
