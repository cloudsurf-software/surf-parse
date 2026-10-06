//! The field module: a block is read as named fields and written back, and
//! the caller never sees block source. Fixtures are three invented sites.

use serde_json::{json, Value};
use surf_parse::fields;

const KAYAK: &str = include_str!("fixtures/fields/kayak.surf");
const BAKERY: &str = include_str!("fixtures/fields/bakery.surf");
const BIKES: &str = include_str!("fixtures/fields/bikes.surf");
/// One canonical instance of every admitted kind (the registry's own snippets, a hero with its headline).
const ALL_KINDS: &str = include_str!("fixtures/fields/all-kinds.surf");

const KINDS: [&str; 17] = [
    "callout", "comparison", "cta", "data", "faq", "features", "form", "gallery", "hero", "infocard", "metric",
    "pricing-table", "product-card", "quote", "stats", "steps", "testimonial",
];

fn stamped(source: &str) -> String {
    fields::handle(&json!({"cmd": "stamp", "source": source})).unwrap()["source"].as_str().unwrap().to_string()
}

fn apply(source: &str, ops: Value) -> Value {
    fields::handle(&json!({"cmd": "apply", "source": source, "ops": ops})).unwrap()
}

fn applied(source: &str, ops: Value) -> String {
    let out = apply(source, ops);
    assert_eq!(out["ok"], true, "refused: {}", out["refused"]);
    out["source"].as_str().unwrap().to_string()
}

fn refusal(source: &str, ops: Value) -> Value {
    let out = apply(source, ops);
    assert_eq!(out["ok"], false);
    assert_eq!(out["source"].as_str().unwrap(), source, "a refused batch returns the input unchanged");
    out["refused"][0].clone()
}

fn read(source: &str, route: &str) -> Value {
    fields::read(source, Some(route)).unwrap()
}

fn block<'a>(page: &'a Value, id: &str) -> &'a Value {
    page["blocks"].as_array().unwrap().iter().find(|b| b["id"] == id).unwrap_or_else(|| panic!("no block {id}"))
}

fn field<'a>(b: &'a Value, name: &str) -> &'a Value {
    &b["fields"].as_array().unwrap().iter().find(|f| f["name"] == name).unwrap_or_else(|| panic!("no field {name}"))["value"]
}

fn items<'a>(b: &'a Value, list: &str) -> &'a Vec<Value> {
    b["lists"].as_array().unwrap().iter().find(|l| l["name"] == list).unwrap()["items"].as_array().unwrap()
}

/// One page that holds every kind, built through `add_page` alone — so the
/// test never writes block source either.
fn every_kind() -> String {
    let row = json!({"Plan": "Day pass", "Price": "$12", "Notes": "Weekdays"});
    let blocks = json!([
        {"kind": "text", "fields": {"heading": "Everything", "body": "One of each kind."}},
        {"kind": "hero", "fields": {"headline": "A quiet harbour", "subtitle": "Small groups only.", "badge": "New"},
         "lists": {"buttons": [{"label": "Book", "href": "/contact", "primary": true}]}},
        {"kind": "cta", "fields": {"label": "Book a tour", "href": "/contact", "primary": true}},
        {"kind": "product-card", "fields": {"title": "Day pass", "body": "One boat, one day.", "price": "$40", "cta_label": "Buy", "cta_href": "/buy"},
         "lists": {"features": [{"text": "Paddle included"}]}},
        {"kind": "form", "fields": {"submit_label": "Send"}, "lists": {"fields": [{"label": "Your name", "required": true}]}},
        {"kind": "callout", "fields": {"type": "tip", "title": "Tide", "body": "Check the tide table."}},
        {"kind": "stats", "lists": {"items": [{"value": "12", "label": "Boats"}]}},
        {"kind": "features", "lists": {"cards": [{"title": "Guides", "body": "Certified."}, {"title": "Gear", "body": "Dry bags."}]}},
        {"kind": "data", "columns": ["Plan", "Price", "Notes"], "lists": {"rows": [row]}},
        {"kind": "testimonial", "fields": {"quote": "Best morning of the trip.", "author": "A paddler", "role": "Visitor"}},
        {"kind": "pricing-table", "columns": ["Plan", "Price", "Notes"], "lists": {"rows": [row]}},
        {"kind": "faq", "lists": {"items": [{"question": "Do I need experience?", "answer": "No."}]}},
        {"kind": "steps", "lists": {"steps": [{"title": "Arrive", "body": "Ten minutes early."}]}},
        {"kind": "metric", "fields": {"label": "Tours run", "value": "340", "trend": "up"}},
        {"kind": "gallery", "lists": {"items": [{"src": "/img/harbour.jpg", "caption": "The harbour", "alt": "Boats at rest"}]}},
        {"kind": "quote", "fields": {"text": "The sea rewards patience.", "attribution": "A guide"}},
        {"kind": "infocard", "fields": {"title": "Find us", "subtitle": "By the old pier"},
         "lists": {"facts": [{"label": "Hours", "value": "Nine to five"}]}},
        {"kind": "comparison", "columns": ["Plan", "Price", "Notes"], "lists": {"rows": [row]}},
    ]);
    applied(&stamped(KAYAK), json!([{"op": "add_page", "route": "/all", "title": "All kinds", "after": null, "blocks": blocks}]))
}

#[test]
fn the_registry_carries_field_rows_for_the_seventeen_kinds() {
    let out = fields::handle(&json!({"cmd": "kinds"})).unwrap();
    let names: Vec<&str> = out["kinds"].as_array().unwrap().iter().map(|k| k["kind"].as_str().unwrap()).collect();
    assert_eq!(names, KINDS);
    assert_eq!(out["crate_version"], surf_parse::spec_registry::CRATE_VERSION);
    let hero = fields::schema("hero").unwrap();
    assert_eq!(hero.fields[0].name, "headline");
    assert_eq!(hero.lists[0].name, "buttons");
    assert!(fields::schema("chart").is_none());
}

#[test]
fn pages_lists_the_site_and_its_routes() {
    let out = fields::pages(KAYAK);
    assert_eq!(out["site"]["name"], "Saltmere Kayak Co");
    assert_eq!(out["site"]["accent"], "#0b6e8a");
    let routes: Vec<&str> = out["pages"].as_array().unwrap().iter().map(|p| p["route"].as_str().unwrap()).collect();
    assert_eq!(routes, ["/", "/pricing", "/contact"]);
    assert_eq!(out["pages"][0]["title"], "Tours");
}

#[test]
fn every_text_field_of_every_site_sets_and_reads_back() {
    let mut writes = 0;
    for site in [KAYAK, BAKERY, BIKES] {
        let src = stamped(site);
        for page in fields::pages(&src)["pages"].as_array().unwrap() {
            let route = page["route"].as_str().unwrap();
            let before = read(&src, route);
            for (n, b) in before["blocks"].as_array().unwrap().iter().enumerate() {
                for (m, f) in b["fields"].as_array().into_iter().flatten().enumerate() {
                    if f["kind"] != "text" {
                        continue;
                    }
                    let out = applied(&src, json!([{"op": "set_fields", "route": route, "block": b["id"], "fields": {f["name"].as_str().unwrap(): "Zq value"}}]));
                    let mut want = before.clone();
                    want["blocks"][n]["fields"][m]["value"] = json!("Zq value");
                    assert_eq!(read(&out, route), want, "{route} {} {}", b["id"], f["name"]);
                    let at = src.find(&format!("id={}", b["id"].as_str().unwrap())).unwrap();
                    let at = src[..at].rfind('\n').unwrap();
                    assert_eq!(out[..at], src[..at], "bytes before the block moved");
                    writes += 1;
                }
            }
        }
    }
    assert!(writes >= 9, "only {writes} field writes were exercised");
}

#[test]
fn a_bad_value_is_refused_by_op_and_field_and_nothing_is_written() {
    let src = stamped(KAYAK);
    let set = |fields: Value| json!({"op": "set_fields", "route": "/", "block": "b-hero-1", "fields": fields});
    let long = "x".repeat(121);
    let r = refusal(&src, json!([set(json!({"headline": long}))]));
    assert_eq!((r["op"].as_u64(), r["field"].as_str(), r["reason"].as_str()), (Some(0), Some("headline"), Some("longer than 120 characters")));
    assert!(refusal(&src, json!([set(json!({"headline": "two\nlines"}))]))["reason"].as_str().unwrap().contains("newline"));
    assert!(refusal(&src, json!([set(json!({"subtitle": "fine\n::\nnot fine"}))]))["reason"].as_str().unwrap().contains("::"));
    assert_eq!(refusal(&src, json!([set(json!({"colour": "red"}))]))["field"], "colour");
    // Every op or none: a good first op does not survive a bad second one.
    let r = refusal(&src, json!([set(json!({"headline": "Fine"})), set(json!({"headline": "a\nb"}))]));
    assert_eq!(r["op"], 1);
    let item = |fields: Value| json!({"op": "set_item", "route": "/", "block": "b-hero-1", "list": "buttons", "index": 0, "fields": fields});
    let r = refusal(&src, json!([item(json!({"href": "javascript:alert(1)"}))]));
    assert_eq!(r["field"], "buttons.href");
    let r = refusal(&src, json!([{"op": "set_item", "route": "/pricing", "block": "b-pricing-table-1", "list": "rows", "index": 0,
                                  "fields": {"Price": "$1 | $2"}}]));
    assert!(r["reason"].as_str().unwrap().contains('|'));
    assert!(refusal(&src, json!([{"op": "set_fields", "block": "b-nope-9", "fields": {}}]))["reason"].is_string());
}

#[test]
fn list_items_are_added_set_moved_and_removed() {
    let src = stamped(KAYAK);
    let page = read(&src, "/");
    let cards = items(block(&page, "b-features-1"), "cards").clone();
    let at = |op: &str, rest: Value| {
        let mut o = json!({"op": op, "route": "/", "block": "b-features-1", "list": "cards"});
        o.as_object_mut().unwrap().extend(rest.as_object().unwrap().clone());
        o
    };
    let out = applied(&src, json!([
        at("add_item", json!({"item": {"title": "Dry bags", "body": "Free with every boat."}, "at": 0})),
        at("set_item", json!({"index": 1, "fields": {"title": "Renamed"}})),
        at("move_item", json!({"from": 0, "to": 1})),
    ]));
    let after = read(&out, "/");
    let now = items(block(&after, "b-features-1"), "cards");
    assert_eq!(now.len(), cards.len() + 1);
    assert_eq!(now[0]["title"], "Renamed");
    assert_eq!(now[0]["body"], cards[0]["body"]);
    assert_eq!(now[1]["title"], "Dry bags");
    let out = applied(&out, json!([at("remove_item", json!({"index": 1}))]));
    assert_eq!(items(block(&read(&out, "/"), "b-features-1"), "cards").len(), cards.len());
    let r = refusal(&src, json!([at("add_item", json!({"item": {"title": "Half a link", "link_label": "More"}}))]));
    assert_eq!(r["field"], "cards.link_label");
    // A table's item fields are its own column headers.
    let table = read(&src, "/pricing");
    let rows = items(block(&table, "b-pricing-table-1"), "rows");
    let header = rows[0].as_object().unwrap().keys().next().unwrap().clone();
    let out = applied(&src, json!([{"op": "set_item", "route": "/pricing", "block": "b-pricing-table-1", "list": "rows", "index": 0,
                                    "fields": {header.clone(): "Zq cell"}}]));
    assert_eq!(items(block(&read(&out, "/pricing"), "b-pricing-table-1"), "rows")[0][&header], "Zq cell");
}

#[test]
fn every_kind_is_built_by_add_page_read_back_and_admitted() {
    let src = every_kind();
    let page = read(&src, "/all");
    let blocks = page["blocks"].as_array().unwrap();
    assert_eq!(blocks[0], json!({"id": null, "kind": "text", "text": "# Everything\n\nOne of each kind."}));
    let kinds: Vec<&str> = blocks[1..].iter().map(|b| b["kind"].as_str().unwrap()).collect();
    let mut sorted = kinds.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, KINDS);
    assert!(blocks[1..].iter().all(|b| b["admitted"] == true && b["id"].is_string()));
    assert_eq!(field(&blocks[1], "headline"), "A quiet harbour");
    let data = blocks.iter().find(|b| b["kind"] == "data").unwrap();
    assert_eq!(items(data, "rows")[0], json!({"Plan": "Day pass", "Price": "$12", "Notes": "Weekdays"}));
    let out = fields::admit(&[("all".to_string(), src)]);
    assert_eq!(out["refused"], json!([]), "refused: {}", out["refused"]);
    assert_eq!(out["admitted"].as_array().unwrap().len(), 17);
}

#[test]
fn blocks_and_pages_are_added_swapped_moved_and_removed() {
    let src = stamped(KAYAK);
    let out = apply(&src, json!([{"op": "add_block", "route": "/", "after": "b-hero-1", "kind": "callout",
                                  "fields": {"type": "note", "title": "Season", "body": "Tours run May to October."}}]));
    assert_eq!(out["touched"], json!([{"route": "/", "id": "b-callout-1"}]), "refused: {}", out["refused"]);
    let src2 = out["source"].as_str().unwrap().to_string();
    let page = read(&src2, "/");
    assert_eq!(page["blocks"][1]["id"], "b-callout-1");
    assert_eq!(field(block(&page, "b-callout-1"), "body"), "Tours run May to October.");
    // A swap keeps the id and carries the fields the two kinds share by name.
    let out = applied(&src2, json!([{"op": "swap_block", "route": "/", "block": "b-callout-1", "kind": "product-card", "fields": {"price": "$40"}}]));
    let card = read(&out, "/");
    let card = block(&card, "b-callout-1");
    assert_eq!((card["kind"].as_str(), field(card, "title").as_str(), field(card, "body").as_str()), (Some("product-card"), Some("Season"), Some("Tours run May to October.")));
    let out = applied(&src2, json!([{"op": "move_block", "route": "/", "block": "b-hero-1", "before": null}]));
    assert_eq!(read(&out, "/")["blocks"].as_array().unwrap().last().unwrap()["id"], "b-hero-1");
    let out = applied(&src2, json!([{"op": "remove_block", "route": "/", "block": "b-callout-1"}]));
    assert_eq!(read(&out, "/"), read(&src, "/"));
    let out = applied(&src, json!([
        {"op": "add_block", "route": "/contact", "after": null, "kind": "cta", "fields": {"label": "Call us", "href": "tel:5550100"}},
        {"op": "set_page", "route": "/pricing", "title": "Rates"},
        {"op": "set_site", "key": "accent", "value": "#224466"},
        {"op": "move_page", "route": "/contact", "before": "/pricing"},
        {"op": "rename_everywhere", "find": "Saltmere", "replace": "Greywater"},
    ]));
    let pages = fields::pages(&out);
    assert_eq!(pages["site"]["name"], "Greywater Kayak Co");
    assert_eq!(pages["site"]["accent"], "#224466");
    let routes: Vec<(&str, &str)> = pages["pages"].as_array().unwrap().iter().map(|p| (p["route"].as_str().unwrap(), p["title"].as_str().unwrap())).collect();
    assert_eq!(routes, [("/", "Tours"), ("/contact", "Contact"), ("/pricing", "Rates")]);
    assert_eq!(read(&out, "/contact")["blocks"][0]["kind"], "cta");
    assert!(!out.contains("Saltmere"));
}

#[test]
fn slot_markers_are_values_like_any_other() {
    let slot = "«FILL: tagline — Small groups, calm water»";
    let src = applied(&stamped(KAYAK), json!([{"op": "set_fields", "route": "/", "block": "b-hero-1", "fields": {"subtitle": slot}}]));
    let out = applied(&src, json!([{"op": "set_fields", "route": "/", "block": "b-hero-1", "fields": {"headline": "New words"}}]));
    let page = read(&out, "/");
    assert_eq!(field(block(&page, "b-hero-1"), "subtitle"), slot);
    assert!(out.contains(slot));
}

#[test]
fn the_fixtures_pass_admission_for_every_kind_with_field_rows() {
    let sources: Vec<(String, String)> = [("kayak", KAYAK), ("bakery", BAKERY), ("bikes", BIKES), ("all-kinds", ALL_KINDS)]
        .iter().map(|(n, s)| (n.to_string(), s.to_string())).collect();
    let out = fields::admit(&sources);
    assert!(out["refused"].as_array().unwrap().is_empty(), "refused: {}", out["refused"]);
    for k in out["kinds"].as_array().unwrap().iter().filter(|k| k["instances"].as_u64() > Some(0)) {
        assert_eq!(k["admitted"], true, "{}: {}", k["kind"], k["refused"]);
        assert_eq!(k["round_trip"], k["instances"]);
        assert_eq!(k["render_same"], k["instances"]);
    }
    assert_eq!(out["admitted"].as_array().unwrap().len(), 17);
}
