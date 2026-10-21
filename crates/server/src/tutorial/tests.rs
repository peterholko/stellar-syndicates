use super::*;

fn steps(reached: &[(Step, Option<f64>)]) -> Vec<Step> {
    reached.iter().map(|(step, _)| *step).collect()
}

fn time(reached: &[(Step, Option<f64>)], step: Step) -> Option<f64> {
    reached.iter().find(|(s, _)| *s == step).and_then(|(_, at)| *at)
}

#[test]
fn a_new_charter_has_only_founding_done() {
    let founding = FoundingProgram::new(10.0, 0, 1, Vec::new());
    let reached = reached_steps(&founding, 20.0);
    assert_eq!(steps(&reached), [Step::Founded]);
    assert_eq!(time(&reached, Step::Founded), Some(10.0));
}

/// §charter-v5: the steps before the raid, as one list.
const BEFORE_RAID: [Step; 5] = [Step::Founded, Step::Bioharvester, Step::Habitat, Step::Agroplex, Step::Mine];

#[test]
fn landfall_facts_count_only_once_their_report_has_arrived() {
    let mut founding = FoundingProgram::new(0.0, 0, 1, Vec::new());
    founding.set_stage(S::PirateRaid, 50.0);
    founding.raid_report_at = Some(80.0);
    founding.privateer_report_at = Some(200.0);
    let reached = reached_steps(&founding, 70.0);
    assert_eq!(steps(&reached), BEFORE_RAID, "the raid's report is still in flight");
    let reached = reached_steps(&founding, 150.0);
    let mut expected = BEFORE_RAID.to_vec();
    expected.push(Step::Raid);
    assert_eq!(steps(&reached), expected, "the privateer's report is still in flight");
    assert_eq!(time(&reached, Step::Mine), Some(50.0));
    assert_eq!(time(&reached, Step::Raid), Some(80.0), "the raid's report, not a stage stamp");
    let reached = reached_steps(&founding, 250.0);
    assert_eq!(time(&reached, Step::Privateer), Some(200.0));
}

#[test]
fn a_later_stage_proves_the_steps_it_requires() {
    let mut founding = FoundingProgram::new(0.0, 0, 1, Vec::new());
    founding.set_stage(S::PirateRaid, 40.0);
    founding.set_stage(S::FirstResearch, 300.0);
    // The reports are not to hand: the Expansion stage still proves Raid and Pirates.
    let reached = reached_steps(&founding, 400.0);
    let mut expected = BEFORE_RAID.to_vec();
    expected.extend([Step::Raid, Step::SellOre, Step::Power, Step::Shipyard, Step::Interceptor,
        Step::Privateer, Step::Den, Step::Shipment]);
    assert_eq!(steps(&reached), expected);
    assert_eq!(time(&reached, Step::Mine), Some(40.0));
    assert_eq!(time(&reached, Step::Privateer), Some(300.0));
    assert!(!steps(&reached).contains(&Step::Research));
}

#[test]
fn a_fulfilled_charter_has_every_step_with_a_time() {
    let mut founding = FoundingProgram::new(0.0, 0, 1, Vec::new());
    // Each stage stamped ten seconds after the last, Complete last.
    for (i, stage) in S::STEPS.into_iter().skip(1).chain([S::Complete]).enumerate() {
        founding.set_stage(stage, 10.0 * (i + 1) as f64);
    }
    let reached = reached_steps(&founding, 1_000.0);
    assert_eq!(steps(&reached), Step::ALL);
    assert!(reached.iter().all(|(_, at)| at.is_some()));
    // A step is proved by the stage after it: the Shipyard (step 8) by step 9.
    assert_eq!(time(&reached, Step::Shipyard), Some(80.0));
    assert_eq!(time(&reached, Step::Survey), Some(150.0));
    assert_eq!(time(&reached, Step::Haul), Some(180.0));
}

#[test]
fn step_keys_round_trip_and_name_the_guide_checklist() {
    for step in Step::ALL {
        assert_eq!(Step::from_key(step.key()), Some(step));
    }
    assert_eq!(Step::Raid.label(), "Pirates raid the colony");
    assert_eq!(Step::SellOre.label(), "Sell ore to the trader");
    assert_eq!(Step::SellOre.chapter(), "Raid");
    assert_eq!(Step::Power.label(), "Power the Shipyard");
    assert_eq!(Step::Power.chapter(), "Raid");
    assert_eq!(Step::Interceptor.chapter(), "Pirates");
    assert_eq!(Step::Shipment.label(), "Export your ore");
    assert_eq!(Step::Shipment.chapter(), "Expansion");
    assert_eq!(Step::Privateer.label(), "Destroy the privateer");
    assert_eq!(Step::Den.label(), "Eliminate the privateer den");
    assert_eq!(Step::Den.chapter(), "Pirates");
    assert_eq!(Step::Research.chapter(), "Expansion");
    assert_eq!(Step::Expansion.label(), "Expand mining operations");
    assert_eq!(Step::Haul.chapter(), "Reach");
    assert_eq!(Step::Haul.label(), "Bring the ore home");
    assert_eq!(Step::ALL.len(), S::STEPS.len() + 1, "one funnel step per charter step, after founding");
    assert_eq!(Stage::all().count(), 21);
    assert_eq!(Step::from_key("beacon"), None, "records from earlier charters are skipped");
    assert_eq!(Step::from_key("semiconductors"), None, "and from the counterfeit charter");
}

fn at(minutes: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_800_000_000, 0).unwrap() + chrono::Duration::minutes(minutes)
}

fn account(signed_up: i64, entered: Option<i64>, done: &[(Step, i64)]) -> AccountTutorial {
    AccountTutorial {
        signed_up: Some(at(signed_up)),
        entered: entered.map(at),
        steps: done.iter().map(|(step, minutes)| (*step, at(*minutes))).collect(),
    }
}

#[test]
fn the_next_step_is_the_first_one_missing_and_waits_since_the_last_progress() {
    let stuck = account(0, Some(5), &[(Step::Founded, 6), (Step::Bioharvester, 7), (Step::Habitat, 8),
        (Step::Agroplex, 9), (Step::Mine, 12), (Step::Raid, 13), (Step::SellOre, 16), (Step::Power, 20),
        (Step::Shipyard, 40)]);
    assert_eq!(stuck.next(), Some(Stage::Step(Step::Interceptor)));
    let summary = summary(&stuck, true);
    assert_eq!(summary.next, Some("interceptor"));
    assert_eq!(summary.next_label, Some("Build an Interceptor"));
    assert_eq!((summary.done, summary.total), (9, 19));
    assert_eq!(summary.since, Some(at(40)));
    assert_eq!(summary.steps.len(), 21);
    assert_eq!(summary.steps[0].key, "signed_up");
    assert!(summary.steps[11].at.is_none());
    assert!(summary_without_steps_is_compact(&stuck));

    let never_entered = account(0, None, &[]);
    assert_eq!(never_entered.next(), Some(Stage::Entered));
    assert_eq!(summary_of(&never_entered).since, Some(at(0)));

    let done = account(0, Some(1), &Step::ALL.map(|step| (step, 2)));
    assert_eq!(done.next(), None);
    assert_eq!(summary_of(&done).since, None);
}

fn summary_of(account: &AccountTutorial) -> PlayerTutorial {
    summary(account, false)
}

fn summary_without_steps_is_compact(account: &AccountTutorial) -> bool {
    let value = serde_json::to_value(summary(account, false)).unwrap();
    value.get("steps").is_none()
}

#[test]
fn the_funnel_counts_who_reached_each_step_and_who_stopped_before_it() {
    let lurker = account(0, None, &[]);
    let landed = |founded: i64, mine: i64| vec![(Step::Founded, founded), (Step::Bioharvester, founded),
        (Step::Habitat, founded), (Step::Agroplex, founded), (Step::Mine, mine)];
    let quitter = account(0, Some(10), &landed(20, 50));
    let playing = account(0, Some(30), &landed(40, 60));
    let finished = account(0, Some(5), &Step::ALL.map(|step| (step, 100)));
    let funnel = funnel(&[
        FunnelAccount { tutorial: &lurker, active: false },
        FunnelAccount { tutorial: &quitter, active: false },
        FunnelAccount { tutorial: &playing, active: true },
        FunnelAccount { tutorial: &finished, active: true },
    ]);
    let row = |key: &str| funnel.iter().find(|stage| stage.key == key).unwrap();
    assert_eq!(funnel.len(), 21);
    assert_eq!(row("signed_up").reached, 4);
    assert_eq!(row("signed_up").median_seconds, None, "nothing comes before signing up");
    assert_eq!(row("entered").reached, 3);
    assert_eq!((row("entered").waiting_active, row("entered").waiting_quiet), (0, 1));
    // Waits from signing up to entering: 5, 10 and 30 minutes.
    assert_eq!(row("entered").median_seconds, Some(600.0));
    assert_eq!(row("mine").reached, 3);
    // Agroplex → mine: 30, 20 and 0 minutes; an even count would average.
    assert_eq!(row("mine").median_seconds, Some(1200.0));
    assert_eq!(row("raid").reached, 1);
    assert_eq!((row("raid").waiting_active, row("raid").waiting_quiet), (1, 1));
    assert_eq!(row("haul").reached, 1);
    assert_eq!(funnel.iter().map(|s| s.waiting_active + s.waiting_quiet).sum::<usize>(), 3, "the finisher waits nowhere");
}
