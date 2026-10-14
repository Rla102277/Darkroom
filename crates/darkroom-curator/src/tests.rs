use crate::store::parse_iso_seconds;
use crate::*;

fn photo(id: &str, created: &str) -> Photo {
    Photo {
        id: id.into(),
        local_id: format!("{id}/L0/001"),
        media: "image".into(),
        width: 4000,
        height: 3000,
        created: Some(created.into()),
        filename: format!("{id}.JPG"),
        ..Default::default()
    }
}

fn measures(sharp: f32, mean: f32, hash: u64) -> Measures {
    Measures { sharp, lap_var: 10f32.powf(sharp * 3.0) - 1.0, mean, black: 0.0, white: 0.0, dhash: format!("{hash:016x}"), modified: None }
}

fn store_of(rows: Vec<(Photo, Option<Measures>)>) -> Store {
    let mut s = Store::new();
    for (p, m) in rows {
        if let Some(m) = m {
            s.measures.insert(p.id.clone(), m);
        }
        s.photos.insert(p.id.clone(), p);
    }
    s
}

#[test]
fn iso_times_parse_with_zones() {
    let z = parse_iso_seconds("2025-03-14T12:00:00Z").unwrap();
    assert_eq!(parse_iso_seconds("2025-03-14T07:00:00-05:00"), Some(z));
    assert_eq!(parse_iso_seconds("2025-03-14T12:00:30.25Z"), Some(z + 30));
    assert_eq!(parse_iso_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_iso_seconds("garbage"), None);
    assert_eq!(parse_iso_seconds("2025-13-14T12:00:00Z"), None);
    assert_eq!(parse_iso_seconds("2025-03-14T12:00:00+0530"), Some(z - 5 * 3600 - 30 * 60));
}

#[test]
fn sharpness_maps_variance_to_a_score() {
    assert_eq!(Measures::sharp_from_variance(0.0), 0.0);
    assert_eq!(Measures::sharp_from_variance(f32::NAN), 0.0);
    assert!((Measures::sharp_from_variance(9.0) - 1.0 / 3.0).abs() < 1e-4);
    assert_eq!(Measures::sharp_from_variance(1e9), 1.0);
}

#[test]
fn store_round_trips_and_refuses_a_newer_version() {
    let dir = std::env::temp_dir().join(format!("curator-store-{}", std::process::id()));
    let path = dir.join(Store::FILE);
    assert_eq!(Store::load(&path).unwrap(), Store::new(), "a missing store is empty");
    let mut s = store_of(vec![(photo("a", "2024-01-01T10:00:00Z"), Some(measures(0.8, 0.5, 7)))]);
    s.notes.push("note".into());
    s.save(&path).unwrap();
    assert_eq!(Store::load(&path).unwrap(), s);
    std::fs::write(&path, br#"{"version": 99}"#).unwrap();
    assert!(Store::load(&path).is_err());
    std::fs::write(&path, b"{not json").unwrap();
    assert!(Store::load(&path).is_err(), "a corrupt store is an error, not a panic");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_fresh_listing_replaces_rows_and_drops_gone_measures() {
    let mut s = store_of(vec![
        (photo("a", "2024-01-01T10:00:00Z"), Some(measures(0.8, 0.5, 7))),
        (photo("b", "2024-01-01T10:00:00Z"), Some(measures(0.8, 0.5, 9))),
    ]);
    let mut a = photo("a", "2024-01-01T10:00:00Z");
    a.favorite = true;
    let (added, changed, removed) = s.replace_photos(vec![a, photo("c", "2024-02-01T10:00:00Z")]);
    assert_eq!((added, changed, removed), (1, 1, 1));
    assert!(s.measures.contains_key("a") && !s.measures.contains_key("b"));
    let need: Vec<&str> = s.needs_measuring().iter().map(|p| p.id.as_str()).collect();
    assert_eq!(need, vec!["c"], "a keeps its measures: not edited since");
}

#[test]
fn bursts_stack_and_your_favorite_wins() {
    let mut rows = Vec::new();
    for i in 0..5 {
        let mut p = photo(&format!("b{i}"), &format!("2024-05-01T10:00:0{i}Z"));
        p.burst_id = Some("BURST".into());
        if i == 3 {
            p.favorite = true;
        }
        // frame 1 is the sharpest, but 3 is the favorite
        rows.push((p, Some(measures(if i == 1 { 0.95 } else { 0.6 }, 0.5, 0x1111_0000_0000_0000 + i as u64 * 0x10_0000))));
    }
    let s = store_of(rows);
    let (stacks, plan) = stacks::build(&s, &Rules::default(), "stacks-1", "now");
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].kind, "burst");
    assert_eq!(stacks[0].winner, "b3");
    assert_eq!(stacks[0].members[1], "b1", "then the sharpest");
    let g = &plan.groups[0];
    assert_eq!(g.key, "stacks.burst");
    assert_eq!(g.items.len(), 4);
    assert!(g.items.iter().all(|i| i.winner.as_deref() == Some("b3") && i.id != "b3"));
    assert!(!g.accepted, "nothing is accepted until you say so");
}

#[test]
fn exact_and_near_duplicates_stack_but_black_frames_do_not() {
    let rows = vec![
        // an import twice: same hash, size, brightness, focus; a year apart
        (photo("x1", "2020-01-01T10:00:00Z"), Some(measures(0.7, 0.4, 0xABCD_EF01_2345_6789))),
        (photo("x2", "2021-01-01T10:00:00Z"), Some(measures(0.7, 0.4, 0xABCD_EF01_2345_6789))),
        // near: 2 bits apart, 20 s apart
        (photo("n1", "2022-06-01T10:00:00Z"), Some(measures(0.8, 0.5, 0x0F0F_0F0F_0F0F_0F0F))),
        (photo("n2", "2022-06-01T10:00:20Z"), Some(measures(0.6, 0.5, 0x0F0F_0F0F_0F0F_0F0C))),
        // the same hash an hour later: not near
        (photo("n3", "2022-06-01T11:00:00Z"), Some(measures(0.6, 0.5, 0x0F0F_0F0F_0F0F_0F0F))),
        // two black frames: identical hashes, no stack
        (
            photo("k1", "2023-01-01T10:00:00Z"),
            Some(Measures { black: 0.99, mean: 0.01, lap_var: 0.1, dhash: "0000000000000000".into(), ..Default::default() }),
        ),
        (
            photo("k2", "2023-01-01T10:00:01Z"),
            Some(Measures { black: 0.99, mean: 0.01, lap_var: 0.1, dhash: "0000000000000000".into(), ..Default::default() }),
        ),
    ];
    let s = store_of(rows);
    let (stacks, plan) = stacks::build(&s, &Rules::default(), "p", "now");
    let kinds: Vec<(&str, usize)> = stacks.iter().map(|s| (s.kind.as_str(), s.members.len())).collect();
    assert!(kinds.contains(&("duplicate", 2)), "{kinds:?}");
    assert!(kinds.contains(&("near", 2)), "{kinds:?}");
    assert_eq!(stacks.len(), 2, "{kinds:?}");
    let near = stacks.iter().find(|s| s.kind == "near").unwrap();
    assert_eq!(near.winner, "n1", "the sharper one");
    assert_eq!(plan.groups.iter().map(|g| g.key.as_str()).collect::<Vec<_>>(), vec!["stacks.duplicate", "stacks.near"]);
}

#[test]
fn cull_rules_each_catch_their_frame_and_spare_favorites() {
    let mut fav = photo("fav", "2024-01-01T10:00:00Z");
    fav.favorite = true;
    let mut shot = photo("shot", "2024-01-01T10:00:00Z");
    shot.screenshot = true;
    let mut kept_shot = photo("docshot", "2024-01-01T10:00:00Z");
    kept_shot.screenshot = true;
    let mut video = photo("vid", "2024-01-01T10:00:00Z");
    video.media = "video".into();
    let rows = vec![
        (photo("black", "2024-01-01T10:00:00Z"), Some(Measures { black: 0.95, mean: 0.01, ..Default::default() })),
        (photo("white", "2024-01-01T10:00:00Z"), Some(Measures { white: 0.95, mean: 0.98, sharp: 0.1, ..Default::default() })),
        (photo("pocket", "2024-01-01T10:00:00Z"), Some(measures(0.2, 0.08, 1))),
        (photo("soft", "2024-01-01T10:00:00Z"), Some(measures(0.2, 0.5, 2))),
        (photo("sharp", "2024-01-01T10:00:00Z"), Some(measures(0.9, 0.5, 3))),
        (fav, Some(Measures { black: 0.95, mean: 0.01, ..Default::default() })),
        (shot, Some(measures(0.9, 0.5, 4))),
        (kept_shot, Some(measures(0.9, 0.5, 5))),
        (video, Some(measures(0.0, 0.0, 6))),
    ];
    let mut s = store_of(rows);
    s.albums.push(Album { id: "A".into(), title: "Receipts".into(), folder: "Admin".into(), members: vec!["docshot".into()] });
    let plan = cull::build(&s, &Rules::default(), "cull-1", "now");
    let got: Vec<(&str, Vec<&str>)> = plan.groups.iter().map(|g| (g.key.as_str(), g.items.iter().map(|i| i.id.as_str()).collect())).collect();
    assert_eq!(
        got,
        vec![
            ("cull.black", vec!["black"]),
            ("cull.white", vec!["white"]),
            ("cull.pocket", vec!["pocket"]),
            ("cull.blur", vec!["soft"]),
            ("cull.screenshot", vec!["shot"]),
        ]
    );
    assert!(plan.groups.iter().all(|g| g.add == vec![stacks::PICK_NO.to_string(), cull::KIND_JUNK.to_string()]));
}

#[test]
fn too_many_near_identical_frames_are_culled_past_the_limit() {
    let mut rows = Vec::new();
    for i in 0..7 {
        rows.push((photo(&format!("f{i}"), &format!("2024-05-01T10:00:0{i}Z")), Some(measures(0.9 - i as f32 * 0.01, 0.5, 0x5555_5555_5555_5555))));
    }
    let mut s = store_of(rows);
    // not exact duplicates (focus differs), so near
    let (st, _) = stacks::build(&s, &Rules::default(), "p", "now");
    assert_eq!(st.len(), 1);
    assert_eq!(st[0].kind, "near");
    s.stacks = st;
    let plan = cull::build(&s, &Rules::default(), "c", "now");
    let g = plan.groups.iter().find(|g| g.key == "cull.nearDupMax").unwrap();
    assert_eq!(g.items.len(), 3, "7 frames, keep the best 4");
    assert!(g.items.iter().all(|i| ["f4", "f5", "f6"].contains(&i.id.as_str())));
}

#[test]
fn a_plan_applies_only_accepted_groups_and_never_what_you_took_out() {
    let mut plan = Plan {
        id: "p".into(),
        job: "cull".into(),
        groups: vec![
            Group {
                key: "a".into(),
                add: vec!["Pick: No".into(), "Kind: Junk".into()],
                items: vec![Item { id: "1".into(), ..Default::default() }, Item { id: "2".into(), excluded: true, ..Default::default() }],
                accepted: true,
                ..Default::default()
            },
            Group {
                key: "b".into(),
                add: vec!["Pick: No".into()],
                items: vec![Item { id: "3".into(), ..Default::default() }],
                accepted: false,
                ..Default::default()
            },
            Group {
                key: "c".into(),
                add: vec!["Pick: No".into()],
                items: vec![Item { id: "1".into(), ..Default::default() }],
                accepted: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert_eq!(plan.changes(), vec![("1".to_string(), vec!["Pick: No".to_string(), "Kind: Junk".to_string()])]);
    let (photos, per) = plan.totals();
    assert_eq!(photos, 1);
    assert_eq!(per.get("Pick: No"), Some(&2), "per group membership");
    plan.groups[1].accepted = true;
    assert_eq!(plan.changes().len(), 2);
}

#[test]
fn the_report_counts_what_the_store_holds() {
    let mut a = photo("a", "2019-07-04T10:00:00Z");
    a.filename = "L1021032.DNG".into();
    a.favorite = true;
    a.lat = Some(33.1);
    let mut b = photo("b", "2019-07-04T10:00:01Z");
    b.filename = "DSCF2826.RAF".into();
    let mut c = photo("c", "2024-01-01T10:00:00Z");
    c.filename = "IMG_0001.HEIC".into();
    c.screenshot = true;
    let mut v = photo("v", "2024-01-01T10:00:00Z");
    v.media = "video".into();
    v.filename = "IMG_0002.MOV".into();
    let mut s = store_of(vec![(a, Some(measures(0.9, 0.5, 0x1234_5678_9ABC_DEF0))), (b, Some(measures(0.1, 0.5, 2))), (c, None), (v, None)]);
    s.albums = vec![
        Album { id: "1".into(), title: "Trip".into(), members: (0..10).map(|i| format!("m{i}")).collect(), ..Default::default() },
        Album { id: "2".into(), title: "Trip copy".into(), members: (0..9).map(|i| format!("m{i}")).collect(), ..Default::default() },
        Album { id: "3".into(), title: "Empty".into(), ..Default::default() },
    ];
    let r = report::build(&s, &Rules::default());
    assert_eq!((r.total, r.images, r.videos, r.favorites, r.located), (4, 3, 1, 1, 1));
    assert_eq!(r.by_year.get("2019"), Some(&2));
    assert_eq!(r.by_device.get("Leica"), Some(&1));
    assert_eq!(r.by_device.get("Fujifilm"), Some(&1));
    assert_eq!(r.by_device.get("iPhone"), Some(&2));
    assert_eq!(r.blur, 1);
    assert_eq!(r.screenshots_outside_documents, 1);
    assert_eq!((r.albums, r.empty_albums, r.in_no_album), (3, 1, 4));
    assert_eq!(r.overlaps.len(), 1);
    assert_eq!(r.overlaps[0].shared, 9);
    assert!(r.notes.iter().any(|n| n.contains("1 of 3 images have no quick measures")));
    let text = r.text();
    assert!(text.contains("4 items: 3 photos, 1 videos"), "{text}");
    assert!(!text.contains('\u{2014}'), "no em dashes");
}

#[test]
fn rules_default_when_missing_and_read_a_partial_file() {
    let dir = std::env::temp_dir().join(format!("curator-rules-{}", std::process::id()));
    let path = dir.join(Rules::FILE);
    assert_eq!(Rules::load(&path).unwrap(), Rules::default());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&path, br#"{"cull": {"blur": 0.2}, "memories": [{"people": ["x"]}]}"#).unwrap();
    let r = Rules::load(&path).unwrap();
    assert_eq!(r.cull.blur, 0.2);
    assert_eq!(r.cull.near_dup_max, 4, "the rest default");
    assert!(r.is_document_album("Admin/Receipts"));
    assert!(!r.is_document_album("Iceland 2026"));
    let _ = std::fs::remove_dir_all(&dir);
}
