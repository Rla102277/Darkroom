use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::Session;

/// A library of six: a burst of three (frame b2 the sharpest), a black frame, a screenshot and a
/// sharp favorite; one album.
fn fake_library(writes: Arc<Mutex<Vec<Value>>>) -> darkroom_photos::Photos {
    darkroom_photos::Photos::with_runner(darkroom_photos::fake_runner(move |cmd, p| match cmd {
        "assets" => {
            assert_eq!(p["hidden"], true, "the Curator sees hidden photos");
            assert_eq!(p["allBursts"], true, "and every burst frame");
            let row = |id: &str, extra: Value| {
                let mut r = json!({"id": format!("{id}/L0/001"), "cloudId": format!("C-{id}"), "mediaType": "image", "width": 4000, "height": 3000,
                    "created": "2025-03-14T10:00:00Z", "filename": format!("{id}.JPG")});
                if let (Some(o), Some(e)) = (r.as_object_mut(), extra.as_object()) {
                    for (k, v) in e {
                        o.insert(k.clone(), v.clone());
                    }
                }
                r
            };
            Ok(json!({"total": 6, "offset": 0, "items": [
                row("b1", json!({"burstId": "B", "created": "2025-03-14T10:00:01Z"})),
                row("b2", json!({"burstId": "B", "created": "2025-03-14T10:00:02Z"})),
                row("b3", json!({"burstId": "B", "created": "2025-03-14T10:00:03Z"})),
                row("dark", json!({"created": "2025-03-15T22:00:00Z"})),
                row("shot", json!({"screenshot": true, "filename": "IMG_0001.PNG"})),
                row("fav", json!({"favorite": true, "created": "2024-01-01T00:00:00Z"})),
            ]}))
        }
        "albums" => Ok(json!({"items": [
            {"id": "F", "title": "Trips", "kind": "folder", "depth": 0, "parent": ""},
            {"id": "A", "title": "Iceland", "kind": "album", "depth": 1, "parent": "F"},
        ]})),
        "album.members" => Ok(json!({"items": [{"id": "A", "title": "Iceland", "ids": ["fav/L0/001", "b1/L0/001"]}]})),
        "curator.measure" => {
            let items: Vec<Value> = p["ids"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|id| {
                    let name = id.split('/').next().unwrap_or("");
                    match name {
                        "dark" => json!({"id": id, "lapVar": 0.5, "mean": 0.01, "black": 0.97, "white": 0.0, "dhash": "0000000000000000"}),
                        "b2" => json!({"id": id, "lapVar": 800.0, "mean": 0.5, "black": 0.0, "white": 0.0, "dhash": "0f0f0f0f0f0f0f0f"}),
                        "b1" | "b3" => json!({"id": id, "lapVar": 300.0, "mean": 0.5, "black": 0.0, "white": 0.0, "dhash": "0f0f0f0f0f0f0f0e"}),
                        "shot" => json!({"id": id, "lapVar": 650.0, "mean": 0.8, "black": 0.0, "white": 0.0, "dhash": "1111222233334444"}),
                        _ => json!({"id": id, "lapVar": 700.0, "mean": 0.45, "black": 0.0, "white": 0.0, "dhash": "123456789abcdef0"}),
                    }
                })
                .collect();
            Ok(json!({"items": items}))
        }
        "keywords.set" => {
            writes.lock().unwrap().push(p.clone());
            Ok(json!({"changed": p["items"].as_array().map_or(0, |a| a.len()), "failed": []}))
        }
        other => Err(darkroom_photos::Error::Helper(format!("unexpected {other}"))),
    }))
}

#[test]
fn scan_report_stacks_cull_review_and_apply() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut s = Session::new();
    s.photos = Some(fake_library(writes.clone()));
    let dir = super::curator_dir(&s);
    let _ = std::fs::remove_dir_all(&dir);

    assert!(s.execute("curator.report", &Value::Null).is_err(), "no report before a scan");
    let scan = s.execute("curator.scan", &json!({"wait": true})).unwrap();
    assert_eq!(scan["photos"], 6);
    assert_eq!(scan["measured"], 6);
    assert_eq!(scan["albums"], 1);
    let status = s.execute("curator.status", &Value::Null).unwrap();
    assert_eq!(status["store"]["photos"], 6);
    assert!(status["job"]["running"] == false && status["job"]["result"]["photos"] == 6);
    assert!(dir.join("curator-rules.json").exists(), "the default rules are written out to edit");

    let report = s.execute("curator.report", &Value::Null).unwrap();
    assert_eq!(report["report"]["bursts"], 1);
    assert_eq!(report["report"]["blackOrBlown"], 1);
    assert_eq!(report["report"]["inNoAlbum"], 4);
    assert!(report["text"].as_str().unwrap().contains("6 items"));

    // stacks: b2 wins its burst, b1 and b3 are proposed
    let st = s.execute("curator.stacks", &Value::Null).unwrap();
    assert_eq!(st["stacks"], 1);
    let page = s.execute("curator.plan", &json!({"job": "stacks", "group": "stacks.burst"})).unwrap();
    let ids: Vec<&str> = page["items"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"C-b1") && ids.contains(&"C-b3"));
    assert!(page["items"].as_array().unwrap().iter().all(|i| i["winner"] == "C-b2"));

    // cull: the black frame and the screenshot; never the favorite
    let cull = s.execute("curator.cull", &Value::Null).unwrap();
    let keys: Vec<&str> = cull["groups"].as_array().unwrap().iter().map(|g| g["key"].as_str().unwrap()).collect();
    assert_eq!(keys, vec!["cull.black", "cull.screenshot"]);
    let cull_id = cull["id"].as_str().unwrap().to_string();

    // review before write: nothing accepted, nothing to write
    let dry = s.execute("curator.apply", &json!({"id": cull_id})).unwrap();
    assert_eq!(dry["wouldWrite"]["photos"], 0);
    assert!(s.execute("curator.apply", &json!({"id": cull_id, "confirm": true, "wait": true})).is_err(), "nothing accepted");
    assert!(writes.lock().unwrap().is_empty(), "nothing reached Photos");

    // accept both groups, then take the screenshot back out
    s.execute("curator.review", &json!({"id": cull_id, "group": "cull.black", "accept": true})).unwrap();
    s.execute("curator.review", &json!({"id": cull_id, "group": "cull.screenshot", "accept": true, "exclude": ["C-shot"]})).unwrap();
    let dry = s.execute("curator.apply", &json!({"id": cull_id})).unwrap();
    assert_eq!(dry["wouldWrite"]["photos"], 1);
    assert!(writes.lock().unwrap().is_empty(), "a dry run writes nothing");

    let done = s.execute("curator.apply", &json!({"id": cull_id, "confirm": true, "wait": true})).unwrap();
    assert_eq!(done["written"], 1);
    let w = writes.lock().unwrap().clone();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0]["items"], json!([{"id": "C-dark", "add": ["Pick: No", "Kind: Junk"]}]), "only keywords are added: no remove, no hide");
    assert!(s.execute("curator.apply", &json!({"id": cull_id, "confirm": true, "wait": true})).is_err(), "an applied plan is not applied twice");
    assert!(s.execute("curator.review", &json!({"id": cull_id, "group": "cull.black", "accept": false})).is_err());

    // the record: one accepted and written, one taken out
    let store = darkroom_curator::Store::load(&dir.join("curator-store.json")).unwrap();
    let d: Vec<(&str, bool, bool)> = store.decisions.iter().map(|d| (d.id.as_str(), d.accepted, d.written)).collect();
    assert_eq!(d, vec![("C-dark", true, true), ("C-shot", false, false)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rescan_measures_only_new_and_edited_photos() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut s = Session::new();
    s.photos = Some(fake_library(writes));
    let dir = std::env::temp_dir().join(format!("darkroom-curator-rescan-{}", std::process::id()));
    let photos = s.photos.clone().unwrap();
    let never = std::sync::atomic::AtomicBool::new(false);
    let first = super::run_scan(&photos, &dir, None, &never, &|_, _, _| {}).unwrap();
    assert_eq!(first["measured"], 6);
    let again = super::run_scan(&photos, &dir, None, &never, &|_, _, _| {}).unwrap();
    assert_eq!((again["measured"].as_u64(), again["toMeasure"].as_u64()), (Some(0), Some(0)));
    // a cancelled listing leaves the store as it was
    let stop = std::sync::atomic::AtomicBool::new(true);
    assert!(super::run_scan(&photos, &dir, None, &stop, &|_, _, _| {}).is_err());
    assert_eq!(darkroom_curator::Store::load(&dir.join("curator-store.json")).unwrap().photos.len(), 6);
    let _ = std::fs::remove_dir_all(&dir);
}
