//! The 0.33.0 backends grammar, end to end, against the MoodMap fixture
//! (`tests/fixtures/backends/moodmap-app.surf`, every new element once).
//!
//! The platform runtime reads the rendered page through `data-*`
//! attributes, so each contract string is pinned here verbatim: a renderer
//! change that moves one of them breaks the runtime, and must break this
//! suite first. surf-parse emits no script for any of these blocks.

use surf_parse::{Block, FieldConstraint, FormFieldType, ModelFieldType};

fn fixture() -> String {
    let path = format!("{}/tests/fixtures/backends/moodmap-app.surf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn html_of(src: &str) -> String {
    surf_parse::parse(src).doc.to_html()
}

#[test]
fn moodmap_parses_every_new_element() {
    let result = surf_parse::parse(&fixture());
    let blocks = &result.doc.blocks;
    let find = |pred: &dyn Fn(&Block) -> bool| blocks.iter().find(|b| pred(b)).cloned();

    // ::model — owner, the new types, the new constraints, the computed field.
    let Some(Block::Model { name, owner, fields, .. }) = find(&|b| matches!(b, Block::Model { .. })) else {
        panic!("no model: {blocks:#?}")
    };
    assert_eq!(name, "Entry");
    assert_eq!(owner, "viewer");
    assert_eq!(fields.len(), 11);
    let field = |n: &str| fields.iter().find(|f| f.name == n).unwrap_or_else(|| panic!("no field {n}"));
    assert_eq!(field("id").field_type, ModelFieldType::Uuid);
    assert_eq!(field("id").constraints, vec![FieldConstraint::Primary], "`pk` is Primary");
    assert_eq!(field("date").field_type, ModelFieldType::Date);
    assert_eq!(field("entry").field_type, ModelFieldType::Ref("Entry".into()));
    assert_eq!(field("core").field_type, ModelFieldType::Enum(vec!["Joy".into(), "Trust".into()]));
    assert_eq!(field("intensity").field_type, ModelFieldType::Int);
    assert_eq!(
        field("intensity").constraints,
        vec![FieldConstraint::Required, FieldConstraint::Min(1), FieldConstraint::Max(10)]
    );
    assert_eq!(field("note").field_type, ModelFieldType::Text, "`textarea` is Text");
    assert_eq!(
        field("note").constraints,
        vec![
            FieldConstraint::Max(500),
            FieldConstraint::Sentences(1, 4),
            FieldConstraint::Prompt("What caused you to feel this way?".into()),
        ]
    );
    assert_eq!(field("level").field_type, ModelFieldType::Range);
    assert_eq!(
        field("level").constraints,
        vec![
            FieldConstraint::Min(1),
            FieldConstraint::Max(10),
            FieldConstraint::Labels(vec!["Barely felt it".into(), "All-consuming".into()]),
            FieldConstraint::Levels(vec!["2".into(), "4".into(), "7".into(), "9".into()]),
        ]
    );
    assert_eq!(field("reminder_time").field_type, ModelFieldType::String);
    assert_eq!(
        field("reminder_time").constraints,
        vec![FieldConstraint::Default("20:00".into()), FieldConstraint::Pattern(r"^\d\d:\d\d$".into())],
        "quoted values are verbatim, quotes stripped"
    );
    assert_eq!(field("score").field_type, ModelFieldType::Float, "`number` is Float");
    assert_eq!(field("score").computed.as_deref(), Some("round(sum(emotions.weighted) / count(emotions), 2)"));
    assert!(field("score").constraints.is_empty());
    assert_eq!(field("boolean_field").field_type, ModelFieldType::Bool);
    assert_eq!(field("boolean_field").constraints, vec![FieldConstraint::Default("false".into())]);
    assert_eq!(field("emotions").field_type, ModelFieldType::List("Emotion".into()));
    assert!(fields.iter().filter(|f| f.name != "score").all(|f| f.computed.is_none()));

    // ::route — filter= / sort= kept; an aggregate returns= verbatim.
    let routes: Vec<&Block> = blocks.iter().filter(|b| matches!(b, Block::Route { .. })).collect();
    assert_eq!(routes.len(), 2);
    match routes[0] {
        Block::Route { path, returns, filter, sort, .. } => {
            assert_eq!(path, "/_api/Entry");
            assert_eq!(returns.as_deref(), Some("list(Entry)"));
            assert_eq!(filter.as_deref(), Some("date"));
            assert_eq!(sort.as_deref(), Some("date"));
        }
        _ => unreachable!(),
    }
    match routes[1] {
        Block::Route { returns, filter, sort, .. } => {
            assert_eq!(
                returns.as_deref(),
                Some("aggregate(avg(score),max(score),min(score),count,streak(date) by month(date))")
            );
            assert_eq!((filter, sort), (&None, &None));
        }
        _ => unreachable!(),
    }

    // ::flow — two steps, the picker nested in the first, a repeatable second.
    let Some(Block::Flow { model, steps, .. }) = find(&|b| matches!(b, Block::Flow { .. })) else {
        panic!("no flow")
    };
    assert_eq!(model.as_deref(), Some("Entry"));
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].title.as_deref(), Some("Pick an emotion"));
    assert!(!steps[0].repeat);
    assert!(steps[0].fields.is_empty());
    let picker = steps[0]
        .children
        .iter()
        .find(|b| matches!(b, Block::Picker { .. }))
        .unwrap_or_else(|| panic!("picker nested in step 1: {:#?}", steps[0].children));
    match picker {
        Block::Picker { bind, tiers, layout, emoji, rows, .. } => {
            assert_eq!(bind, "entry.core");
            assert_eq!(*tiers, 3);
            assert_eq!(layout, "wheel");
            assert!(!emoji);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].core, "Joy");
            assert_eq!(rows[0].choices, vec!["Serenity", "Joy", "Ecstasy"]);
            assert_eq!(rows[0].info.as_deref(), Some("Feeling content and at ease | Seeing an old friend"));
            assert_eq!(rows[1].core, "Trust");
            assert_eq!(rows[1].info, None);
        }
        _ => unreachable!(),
    }
    assert_eq!(steps[1].title.as_deref(), Some("What caused it?"));
    assert!(steps[1].repeat);
    assert_eq!(steps[1].fields.len(), 1);
    assert_eq!(steps[1].fields[0].name, "note");
    assert_eq!(steps[1].fields[0].label, "Note");
    assert_eq!(steps[1].fields[0].field_type, FormFieldType::Textarea);
    assert_eq!(steps[1].fields[0].placeholder.as_deref(), Some("What caused you to feel this way?"));

    // ::form[model=] — a model field line becomes a range field.
    let Some(Block::Form { model, fields, action, .. }) = find(&|b| matches!(b, Block::Form { .. })) else {
        panic!("no form")
    };
    assert_eq!(model.as_deref(), Some("Entry"));
    assert_eq!(action, None, "the model route is derived at render time");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "level");
    assert_eq!(fields[0].field_type, FormFieldType::Range);

    // ::compute, ::when, ::schedule.
    match find(&|b| matches!(b, Block::Compute { .. })) {
        Some(Block::Compute { name, expr, source, .. }) => {
            assert_eq!(name, "daily_score");
            assert_eq!(expr, "avg(entries.weighted)");
            assert_eq!(source.as_deref(), Some("/_api/Entry"));
        }
        other => panic!("no compute: {other:?}"),
    }
    match find(&|b| matches!(b, Block::When { .. })) {
        Some(Block::When { bind, op, value, expr, children, .. }) => {
            assert_eq!(bind.as_deref(), Some("day.score"));
            assert_eq!(op.as_deref(), Some("<="));
            assert_eq!(value.as_deref(), Some("-8"));
            assert_eq!(expr, None);
            assert!(!children.is_empty());
        }
        other => panic!("no when: {other:?}"),
    }
    match find(&|b| matches!(b, Block::Schedule { .. })) {
        Some(Block::Schedule { bind, tz, title, body, link, .. }) => {
            assert_eq!(bind, "preference.reminder_time");
            assert_eq!(tz, "viewer");
            assert_eq!(title.as_deref(), Some("MoodMap"));
            assert_eq!(body.as_deref(), Some("How are you feeling today?"));
            assert_eq!(link.as_deref(), Some("/log"));
        }
        other => panic!("no schedule: {other:?}"),
    }
}

/// Every `data-*` contract string the platform runtime reads, verbatim.
#[test]
fn moodmap_html_carries_the_data_contract() {
    let html = html_of(&fixture());
    let contract = [
        // ::when — the predicate as data attributes, hidden until it holds.
        "<section class=\"surfdoc-when\" data-when-bind=\"day.score\" data-when-op=\"&lt;=\" data-when-value=\"-8\" hidden><p>A hard day. Reach out to someone you trust.</p>",
        // ::compute — an empty output the runtime fills.
        "<output class=\"surfdoc-compute\" data-compute-name=\"daily_score\" data-compute-expr=\"avg(entries.weighted)\" data-source=\"/_api/Entry\"></output>",
        // ::picker — the fieldset, a row with its info, one radio per tier.
        "<fieldset class=\"surfdoc-picker\" data-bind=\"entry.core\" data-tiers=\"3\" data-layout=\"wheel\" data-emoji=\"false\">",
        "<div class=\"surfdoc-picker-row\" data-core=\"Joy\" data-info=\"Feeling content and at ease | Seeing an old friend\">",
        "<label class=\"surfdoc-picker-choice\"><input type=\"radio\" name=\"entry.core\" value=\"Serenity\" data-tier=\"1\"/>Serenity</label>",
        "<input type=\"radio\" name=\"entry.core\" value=\"Joy\" data-tier=\"2\"/>",
        "<input type=\"radio\" name=\"entry.core\" value=\"Ecstasy\" data-tier=\"3\"/>",
        "<div class=\"surfdoc-picker-row\" data-core=\"Trust\"><label class=\"surfdoc-picker-choice\"><input type=\"radio\" name=\"entry.core\" value=\"Acceptance\" data-tier=\"1\"/>",
        // ::flow — the stepped form bound to the record; step 2 repeats.
        "<form class=\"surfdoc-form\" method=\"post\" action=\"/_api/Entry\" data-model=\"Entry\" data-steps=\"true\">",
        "<input type=\"radio\" name=\"_step\" value=\"1\" id=\"flow-step-1\" class=\"surfdoc-form-step-radio\" checked/>",
        "<fieldset class=\"surfdoc-form-step\" data-step=\"1\"><legend>Step 1 of 2 \u{b7} Pick an emotion</legend>",
        "<fieldset class=\"surfdoc-form-step\" data-step=\"2\" data-repeat=\"true\"><legend>Step 2 of 2 \u{b7} What caused it?</legend>",
        "<button type=\"submit\" class=\"surfdoc-form-submit\">Save</button></div></fieldset></form>",
        // A textarea with sentences= and prompt=.
        "<textarea name=\"note\" placeholder=\"What caused you to feel this way?\" rows=\"4\" maxlength=\"500\" data-sentences=\"1..4\"></textarea>",
        // ::form[model=] — posts one row; a range field with its labels and levels.
        "<form class=\"surfdoc-form\" method=\"post\" action=\"/_api/Entry\" data-model=\"Entry\">",
        "<input type=\"range\" name=\"level\" min=\"1\" max=\"10\" step=\"1\" data-labels=\"Barely felt it|All-consuming\" data-levels=\"2|4|7|9\"/><output class=\"surfdoc-form-range-value\"></output>",
        // ::schedule — the reminder's description, hidden.
        "<span class=\"surfdoc-schedule\" data-schedule-bind=\"preference.reminder_time\" data-schedule-tz=\"viewer\" data-schedule-title=\"MoodMap\" data-schedule-body=\"How are you feeling today?\" data-schedule-link=\"/log\" hidden></span>",
        // ::model — the computed field and the quoted constraints, escaped.
        "<td><code>float</code> <span class=\"surfdoc-model-computed\">= round(sum(emotions.weighted) / count(emotions), 2)</span></td>",
        // A default the unquoted reading keeps is written bare; a pattern
        // always quoted.
        "[default=20:00, pattern=&quot;^\\d\\d:\\d\\d$&quot;]",
        // ::route — filter / sort as detail rows.
        "<li>filter: <code>date</code></li><li>sort: <code>date</code></li>",
    ];
    for want in contract {
        assert!(html.contains(want), "missing contract string:\n{want}\n\nin:\n{html}");
    }
    assert!(!html.contains("<script"), "the backends blocks emit no script");
}

#[test]
fn when_expr_form_and_defaults() {
    let html = html_of("::when[expr=\"day.score <= -8\"]\nBody\n::\n");
    assert!(html.contains("<section class=\"surfdoc-when\" data-when-expr=\"day.score &lt;= -8\" hidden>"), "{html}");
    let html = html_of("::picker[bind=a.b]\n- Calm: Still · Calm\n- Joy\n::\n");
    // Default tiers = the widest row; default layout wheel; emoji on.
    assert!(html.contains("data-bind=\"a.b\" data-tiers=\"2\" data-layout=\"wheel\" data-emoji=\"true\""), "{html}");
    // A row without a colon is its own core.
    assert!(html.contains("<div class=\"surfdoc-picker-row\" data-core=\"Joy\"><label class=\"surfdoc-picker-choice\"><input type=\"radio\" name=\"a.b\" value=\"Joy\" data-tier=\"1\"/>Joy</label></div>"), "{html}");
    let html = html_of("::schedule[bind=p.t]\n::\n");
    assert!(html.contains("<span class=\"surfdoc-schedule\" data-schedule-bind=\"p.t\" data-schedule-tz=\"viewer\" hidden></span>"), "{html}");
    // A flow without id= or submit= uses the `flow` radio prefix and "Save".
    let html = html_of("::flow\n:::step\n- name: string\n:::\n::\n");
    assert!(html.contains("<form class=\"surfdoc-form\" data-steps=\"true\">"), "no model, no action: {html}");
    assert!(html.contains("id=\"flow-step-1\""));
    assert!(html.contains("<legend>Step 1 of 1</legend>"));
}

/// Script schemes in a URL-ish attribute become `#`; quote-breaking text is
/// escaped; a model name cannot steer the action anywhere but `/_api/`.
#[test]
fn hostile_values_are_neutralised_and_escaped() {
    let html = html_of(
        "::schedule[bind=p.t link=\"javascript:alert(1)\" title=\"a\\\"><script>x</script>\"]\n::\n\n\
         ::compute[name=n expr=\"x\" source=\" JaVaScRiPt:alert(1)\"]\n::\n\n\
         ::compute[name=m expr=\"y\" source=\"data:text/html,<b>\"]\n::\n\n\
         ::form[model=\"../evil?x=javascript:1\"]\n- level: range\n::\n\n\
         ::picker[bind=\"a\\\" onclick=\\\"x\"]\n- Joy: <b>Serenity</b> | info=\"<img src=x>\"\n::\n",
    );
    assert!(html.contains("data-schedule-link=\"#\""), "{html}");
    assert!(html.contains("data-schedule-title=\"a&quot;&gt;&lt;script&gt;x&lt;/script&gt;\""), "{html}");
    assert_eq!(html.matches("data-source=\"#\"").count(), 2, "{html}");
    assert!(html.contains("action=\"/_api/evilxjavascript1\""), "{html}");
    assert!(html.contains("data-bind=\"a&quot; onclick=&quot;x\""), "{html}");
    assert!(html.contains("value=\"&lt;b&gt;Serenity&lt;/b&gt;\""), "{html}");
    assert!(html.contains("data-info=\"&lt;img src=x&gt;\""), "{html}");
    assert!(!html.contains("<script"), "{html}");
    assert!(!html.contains("<img src=x>"), "{html}");
}

/// A form without model= is byte-for-byte the 0.32 form; with model= it
/// posts to the model route, and an authored action= still wins.
#[test]
fn form_model_binding_and_the_unchanged_plain_form() {
    let plain = html_of("::form[submit=\"Send\"]\ngroup: Contact\n- Name (text) *\n::");
    assert!(plain.contains("<form class=\"surfdoc-form\"><fieldset class=\"surfdoc-form-group\"><legend>Contact</legend><div class=\"surfdoc-form-field\"><label>Name <span class=\"required\">*</span></label><input type=\"text\" name=\"name\" placeholder=\"\" required/></div></fieldset><button type=\"submit\" class=\"surfdoc-form-submit\">Send</button></form>"), "{plain}");
    let own = html_of("::form[model=Entry action=/custom]\n- level: range\n::");
    assert!(own.contains("<form class=\"surfdoc-form\" method=\"post\" action=\"/custom\" data-model=\"Entry\">"), "{own}");
    // No bounds authored: the browser's own 0..100, written out.
    assert!(own.contains("<input type=\"range\" name=\"level\" min=\"0\" max=\"100\" step=\"1\"/>"), "{own}");
    // A model form still reads the form grammar, and a colon shorthand whose
    // right side is not a model type falls through to it.
    let mixed = html_of("::form[model=Entry]\n- Full name (text) *\n- email: Your email\n- when: date [required]\n::");
    assert!(mixed.contains("<input type=\"text\" name=\"full_name\" placeholder=\"\" required/>"), "{mixed}");
    assert!(mixed.contains("<label>Your email</label><input type=\"email\" name=\"your_email\""), "{mixed}");
    assert!(mixed.contains("<label>When <span class=\"required\">*</span></label><input type=\"date\" name=\"when\" placeholder=\"\" required/>"), "{mixed}");
    // Stepped + model: the model route and data-model on the stepped form.
    let stepped = html_of("::form[model=Entry steps=true]\n:::step[title=\"One\"]\n- level: range [min=1, max=5]\n:::\n::");
    assert!(stepped.contains("<form class=\"surfdoc-form\" method=\"post\" action=\"/_api/Entry\" data-model=\"Entry\" data-steps=\"true\">"), "{stepped}");
}

/// Constraint parsing: quote-aware splitting, `\"` inside a quoted value,
/// bare trailing constraint words, and the unquoted default read as before.
#[test]
fn model_constraint_grammar() {
    let src = "::model[name=T owner=workspace]\n- id: uuid pk auto\n- q: text [prompt=\"Why, today?\", max=10]\n- r: string [prompt=\"Say \\\"hi\\\"\"]\n- s: string [default=Todo]\n- t: string [default=\"Todo\"]\n- u: text [sentences=3]\n- v: enum(a, b) required\n::\n";
    let doc = surf_parse::parse(src).doc;
    let Block::Model { owner, fields, .. } = &doc.blocks[0] else { panic!("{:?}", doc.blocks) };
    assert_eq!(owner, "workspace");
    assert_eq!(fields[0].constraints, vec![FieldConstraint::Primary, FieldConstraint::Auto]);
    assert_eq!(fields[0].field_type, ModelFieldType::Uuid);
    assert_eq!(fields[1].constraints, vec![FieldConstraint::Prompt("Why, today?".into()), FieldConstraint::Max(10)]);
    assert_eq!(fields[2].constraints, vec![FieldConstraint::Prompt("Say \"hi\"".into())]);
    assert_eq!(fields[3].constraints, vec![FieldConstraint::Default("todo".into())], "unquoted: read lower-cased, as before");
    assert_eq!(fields[4].constraints, vec![FieldConstraint::Default("Todo".into())], "quoted: verbatim");
    assert_eq!(fields[5].constraints, vec![FieldConstraint::Sentences(3, 3)]);
    assert_eq!(fields[6].field_type, ModelFieldType::Enum(vec!["a".into(), "b".into()]));
    assert_eq!(fields[6].constraints, vec![FieldConstraint::Required]);
}

/// The markdown degradation says what each block asks for.
#[test]
fn moodmap_markdown_degradation() {
    let md = surf_parse::parse(&fixture()).doc.to_markdown();
    for want in [
        "| score | float = round(sum(emotions.weighted) / count(emotions), 2) |",
        "labels=\"Barely felt it\\|All-consuming\"",
        "- filter: `date`",
        "**Choose** `entry.core`",
        "- **Joy**: Serenity \u{b7} Joy \u{b7} Ecstasy \u{2014} Feeling content and at ease | Seeing an old friend",
        "**New Entry**",
        "**2. What caused it?**",
        "**daily_score** = `avg(entries.weighted)` (from `/_api/Entry`)",
        "*Shown when `day.score <= -8`*",
        "*Reminder: MoodMap \u{2014} How are you feeling today? (daily at `preference.reminder_time`, viewer time)*",
    ] {
        assert!(md.contains(want), "missing {want:?} in:\n{md}");
    }
    assert!(!md.contains("::"), "no directive markers survive:\n{md}");
}

/// The MoodMap fixture is lint-clean of L020 (every block registered) and of
/// L046 (every word the runtime acts on is one it knows).
#[test]
fn moodmap_lints_clean_of_l020_and_l046() {
    let report = surf_parse::check(&fixture());
    let codes: Vec<&str> = report
        .diagnostics
        .iter()
        .filter_map(|d| d.code.as_deref())
        .filter(|c| *c == "L020" || *c == "L046")
        .collect();
    assert!(codes.is_empty(), "unexpected: {:?}", report.diagnostics);
}

/// L046 names each hole: an unknown owner, a predicate-less ::when, an
/// unknown operator, a ::flow without model=, an unknown picker layout and a
/// row wider than tiers=.
#[test]
fn l046_names_the_backends_holes() {
    let src = "::model[name=A owner=everyone]\n- id: uuid pk\n::\n\n::when\nx\n::\n\n::when[bind=a.b op=\"=~\" value=1]\ny\n::\n\n::flow\n:::step\n- n: string\n:::\n::\n\n::picker[bind=a.c tiers=1 layout=carousel]\n- Joy: A · B\n::\n";
    let report = surf_parse::check(src);
    let l046: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|d| d.code.as_deref() == Some("L046"))
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(l046.len(), 6, "{l046:#?}");
    assert!(l046.iter().any(|m| m.contains("owner='everyone'")));
    assert!(l046.iter().any(|m| m.contains("'::when' has no predicate")));
    assert!(l046.iter().any(|m| m.contains("op='=~'")));
    assert!(l046.iter().any(|m| m.contains("'::flow' names no model=")));
    assert!(l046.iter().any(|m| m.contains("layout='carousel'")));
    assert!(l046.iter().any(|m| m.contains("row 'Joy' has 2 choices but tiers=1")));
    assert!(report.diagnostics.iter().filter(|d| d.code.as_deref() == Some("L046")).all(|d| d.severity == surf_parse::Severity::Warning));
}

/// The serializer writes each new block back so the second pass equals the
/// first, and the HTML never moves across the round trip.
#[test]
fn moodmap_serializes_to_a_fixed_point() {
    let src = fixture();
    let first = surf_parse::builder::to_surf_source(&surf_parse::parse(&src).doc);
    let second = surf_parse::builder::to_surf_source(&surf_parse::parse(&first).doc);
    assert_eq!(first, second, "not a fixed point; first pass:\n{first}");
    let html0 = surf_parse::parse(&src).doc.to_html_fragment();
    let html1 = surf_parse::parse(&first).doc.to_html_fragment();
    assert_eq!(html0, html1, "the serializer changed the render; first pass:\n{first}");
    for want in [
        "::model[name=\"Entry\"]",
        "- id: uuid [primary]",
        "- score: float = round(sum(emotions.weighted) / count(emotions), 2)",
        "- reminder_time: string [default=20:00, pattern=\"^\\d\\d:\\d\\d$\"]",
        "::route[method=GET path=\"/_api/Entry\" filter=\"date\" sort=\"date\"]",
        "::flow[model=\"Entry\"]",
        ":::step[title=\"What caused it?\" repeat=true]",
        "- note: textarea [max=500, sentences=1..4, prompt=\"What caused you to feel this way?\"]",
        "::::picker[bind=\"entry.core\" tiers=3 emoji=false]",
        "- Joy: Serenity \u{b7} Joy \u{b7} Ecstasy | info=\"Feeling content and at ease | Seeing an old friend\"",
        "::form[model=\"Entry\"]",
        "- level: range [min=1, max=10, labels=\"Barely felt it|All-consuming\", levels=\"2|4|7|9\"]",
        "::when[bind=\"day.score\" op=\"<=\" value=\"-8\"]",
        "::schedule[bind=\"preference.reminder_time\" title=\"MoodMap\" body=\"How are you feeling today?\" link=\"/log\"]",
    ] {
        assert!(first.contains(want), "missing {want:?} in:\n{first}");
    }
}

#[cfg(feature = "native")]
mod native {
    use super::fixture;
    use surf_parse::render_native::{block_tier, to_native_blocks, BlockTier, NativeBlock, NATIVE_DOC_SCHEMA_VERSION};

    #[test]
    fn moodmap_crosses_the_native_schema() {
        // The backends grammar crossed at v14 (0.33.0); the schema has moved
        // on since (v15, 0.37.0 video; v16, 0.40.0 segment icon + tint) and
        // its variants still cross.
        assert_eq!(NATIVE_DOC_SCHEMA_VERSION, 16);
        let doc = surf_parse::parse(&fixture()).doc;
        for b in doc.blocks.iter().filter(|b| {
            matches!(
                b,
                surf_parse::Block::When { .. }
                    | surf_parse::Block::Compute { .. }
                    | surf_parse::Block::Flow { .. }
                    | surf_parse::Block::Schedule { .. }
            )
        }) {
            assert_eq!(block_tier(b), BlockTier::Chrome, "{b:?}");
        }
        let native = to_native_blocks(&doc);
        let model = native.iter().find_map(|b| match b {
            NativeBlock::Model { owner, fields, .. } => Some((owner.clone(), fields.clone())),
            _ => None,
        });
        let (owner, fields) = model.expect("model crosses");
        assert_eq!(owner, "viewer");
        let score = fields.iter().find(|f| f.name == "score").expect("score");
        assert_eq!(score.field_type, "float");
        assert_eq!(score.computed.as_deref(), Some("round(sum(emotions.weighted) / count(emotions), 2)"));
        let level = fields.iter().find(|f| f.name == "level").expect("level");
        assert_eq!(level.field_type, "range");
        assert_eq!(
            level.constraints,
            vec!["min=1", "max=10", "labels=\"Barely felt it|All-consuming\"", "levels=\"2|4|7|9\""]
        );

        let flow = native.iter().find_map(|b| match b {
            NativeBlock::Flow { model, submit_label, steps } => Some((model.clone(), submit_label.clone(), steps.clone())),
            _ => None,
        });
        let (model, submit, steps) = flow.expect("flow crosses");
        assert_eq!(model.as_deref(), Some("Entry"));
        assert_eq!(submit, "Save");
        assert_eq!(steps.len(), 2);
        assert!(steps[1].repeat);
        assert!(matches!(
            steps[0].children.iter().find(|c| matches!(c, NativeBlock::Picker { .. })),
            Some(NativeBlock::Picker { tiers: 3, emoji: false, .. })
        ));
        assert_eq!(steps[1].fields[0].field_type, "textarea");

        assert!(native.iter().any(|b| matches!(b, NativeBlock::Form { model: Some(m), .. } if m == "Entry")));
        assert!(native.iter().any(|b| matches!(
            b,
            NativeBlock::Route { filter: Some(f), sort: Some(s), .. } if f == "date" && s == "date"
        )));
        assert!(native.iter().any(|b| matches!(b, NativeBlock::Compute { name, .. } if name == "daily_score")));
        assert!(native.iter().any(|b| matches!(
            b,
            NativeBlock::When { op: Some(op), value: Some(v), children, .. } if op == "<=" && v == "-8" && !children.is_empty()
        )));
        assert!(native.iter().any(|b| matches!(b, NativeBlock::Schedule { tz, .. } if tz == "viewer")));
    }
}
