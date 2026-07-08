//! Conformance tests against the reference Python output.
//!
//! The correctness bar is "valid file i2 opens", so for the full examples we
//! assert well-formedness and structural counts rather than byte-equality. For
//! the minimal no-config chart, however, our id-minting and radial layout match
//! upstream exactly, so we can assert the full string — a strong regression
//! anchor.

use anxwritter::builder::Builder;
use anxwritter::input::{ChartData, Config};

const MINIMAL_INPUT: &str = r#"{
  "entities": { "icons": [
    {"id": "alice", "type": "Person", "label": "Alice", "attributes": {"phone": "555-0001"}},
    {"id": "bob", "type": "Person"}
  ] },
  "links": [ {"from_id": "alice", "to_id": "bob", "type": "Call", "arrow": "->"} ]
}"#;

const MINIMAL_EXPECTED: &str = concat!(
    "<?xml version='1.0' encoding='utf-8'?>\n",
    "<!-- Built with anxwritter 1.25.0 — https://github.com/gustavo-gkmi/anxwritter -->\n",
    "<Chart>\n",
    "<ApplicationVersion Major=\"9\" Minor=\"0\" Point=\"0\" Build=\"0\"/>\n",
    "<StrengthCollection>\n",
    "<Strength Id=\"ID1\" Name=\"Default\" DotStyle=\"DotStyleSolid\"/>\n",
    "</StrengthCollection>\n",
    "<AttributeClassCollection>\n",
    "<AttributeClass Id=\"ID3\" Name=\"phone\" Type=\"AttText\" IsUser=\"true\" UserCanAdd=\"true\" UserCanRemove=\"true\" ShowValue=\"true\"/>\n",
    "</AttributeClassCollection>\n",
    "<LinkTypeCollection>\n",
    "<LinkType Id=\"ID5\" Name=\"Call\"/>\n",
    "</LinkTypeCollection>\n",
    "<ChartItemCollection>\n",
    "<ChartItem Id=\"ID2\" Label=\"Alice\" XPosition=\"-80\">\n",
    "<End X=\"-80\" Y=\"320\" Z=\"0\">\n",
    "<Entity EntityId=\"1\" Identity=\"alice\" LabelIsIdentity=\"false\">\n",
    "<Icon>\n",
    "<IconStyle Type=\"Person\"/>\n",
    "</Icon>\n",
    "</Entity>\n",
    "</End>\n",
    "<AttributeCollection>\n",
    "<Attribute AttributeClass=\"phone\" AttributeClassReference=\"ID3\" Value=\"555-0001\"/>\n",
    "</AttributeCollection>\n",
    "</ChartItem>\n",
    "<ChartItem Id=\"ID4\" Label=\"bob\" XPosition=\"80\">\n",
    "<End X=\"80\" Y=\"320\" Z=\"0\">\n",
    "<Entity EntityId=\"2\" Identity=\"bob\" LabelIsIdentity=\"true\">\n",
    "<Icon>\n",
    "<IconStyle Type=\"Person\"/>\n",
    "</Icon>\n",
    "</Entity>\n",
    "</End>\n",
    "</ChartItem>\n",
    "<ChartItem Id=\"ID6\" Label=\"\">\n",
    "<Link End1Id=\"1\" End2Id=\"2\">\n",
    "<LinkStyle Strength=\"Default\" ArrowStyle=\"ArrowOnHead\" Type=\"Call\" LinkTypeReference=\"ID5\"/>\n",
    "</Link>\n",
    "</ChartItem>\n",
    "</ChartItemCollection>\n",
    "<PaletteCollection>\n",
    "<Palette Name=\"anxwritter\">\n",
    "<AttributeClassEntryCollection>\n",
    "<AttributeClassEntry AttributeClass=\"phone\" AttributeClassReference=\"ID3\"/>\n",
    "</AttributeClassEntryCollection>\n",
    "<LinkTypeEntryCollection>\n",
    "<LinkTypeEntry LinkType=\"Call\" LinkTypeReference=\"ID5\"/>\n",
    "</LinkTypeEntryCollection>\n",
    "</Palette>\n",
    "</PaletteCollection>\n",
    "</Chart>\n",
);

#[test]
fn minimal_chart_matches_upstream_byte_for_byte() {
    let data: ChartData = serde_json::from_str(MINIMAL_INPUT).unwrap();
    let xml = Builder::new(&Config::default()).build(&data);
    assert_eq!(xml, MINIMAL_EXPECTED);
}

#[test]
fn evidence_cards_emit_card_collection() {
    let data: ChartData = serde_json::from_str(
        r#"{"entities":{"icons":[{"id":"a","type":"P","cards":[
            {"summary":"Sighting","date":"2026-01-05","description":"Seen downtown","source_type":"Witness"}
        ]}]},"links":[]}"#,
    )
    .unwrap();
    let xml = Builder::new(&Config::default()).build(&data);
    assert!(xml.contains("<CardCollection>"));
    assert!(xml.contains(
        "<Card Summary=\"Sighting\" DateSet=\"true\" DateTime=\"2026-01-05T00:00:00.000\" SourceType=\"Witness\" Text=\"Seen downtown\"/>"
    ));
}

#[test]
fn semantic_types_emit_library_catalogue() {
    let config: Config = serde_json::from_str(
        r#"{"semantic_entities":[
            {"name":"Suspect","kind_of":"Entity","description":"under investigation"}],
           "entity_types":[{"name":"Person","semantic_type":"Suspect"}]}"#,
    )
    .unwrap();
    let data: ChartData = serde_json::from_str(
        r#"{"entities":{"icons":[{"id":"e1","type":"Person","semantic_type":"Suspect"}]},"links":[]}"#,
    )
    .unwrap();
    let xml = Builder::new(&config).build(&data);
    // Deterministic GUID for entity:Suspect.
    let guid = "guidCEE90BAD-62EE-5F57-8F82-E528A0E54AF4";
    assert!(xml.contains("xmlns:lcx="));
    assert!(xml.contains("<lcx:LibraryCatalogue"));
    assert!(xml.contains(&format!("<lcx:Type tGUID=\"{guid}\"")));
    assert!(xml.contains(&format!("SemanticTypeGuid=\"{guid}\"")));
}

#[test]
fn icon_map_overrides_type_icon() {
    let config: Config = serde_json::from_str(
        r#"{"settings":{"extra_cfg":{"icon_map":{"rules":[
            {"match":"attribute","attribute_name":"Role","mapping":{"boss":"crown"},"default":"question"}]}}}}"#,
    )
    .unwrap();
    let data: ChartData = serde_json::from_str(
        r#"{"entities":{"icons":[
            {"id":"a","type":"P","attributes":{"Role":"boss"}},
            {"id":"b","type":"P","attributes":{"Role":"other"}},
            {"id":"c","type":"P"}]},"links":[]}"#,
    )
    .unwrap();
    let xml = Builder::new(&config).build(&data);
    assert!(xml.contains("OverrideTypeIcon=\"true\" TypeIconName=\"crown\""));
    assert!(xml.contains("TypeIconName=\"question\"")); // unrecognised value -> default
                                                        // Entity c has no Role attribute and no default_when_absent -> no override.
    assert_eq!(xml.matches("OverrideTypeIcon").count(), 2);
}

#[test]
fn intensity_styling_scales_width_and_color() {
    let config: Config = serde_json::from_str(
        r#"{"settings":{"extra_cfg":{"styling":{"links":{"intensity":{
            "attribute":"weight","width":{"range":[1,5]},"color":{"ramp":["Blue","Red"],"space":"rgb"}}}}}}}"#,
    )
    .unwrap();
    let data: ChartData = serde_json::from_str(
        r#"{"entities":{"icons":[{"id":"a","type":"P"},{"id":"b","type":"P"}]},
            "links":[
              {"from_id":"a","to_id":"b","type":"L","attributes":{"weight":0}},
              {"from_id":"a","to_id":"b","type":"L","attributes":{"weight":50}},
              {"from_id":"a","to_id":"b","type":"L","attributes":{"weight":100}}]}"#,
    )
    .unwrap();
    let xml = Builder::new(&config).build(&data);
    // Domain [0,100], sqrt scale: weight=100 -> t=1 -> width 5, color Red (255);
    // weight=50 -> t≈0.707 -> width 4.
    assert!(xml.contains("LineWidth=\"5\""));
    assert!(xml.contains("LineWidth=\"4\""));
    assert!(xml.contains("LineColour=\"255\""));
}

#[test]
fn geo_map_projects_positions() {
    let config: Config = serde_json::from_str(
        r#"{"settings":{"extra_cfg":{"geo_map":{"attribute_name":"City","mode":"position",
            "width":2000,"height":1000,"data":{"london":[51.5,-0.13],"paris":[48.85,2.35],"berlin":[52.52,13.4]}}}}}"#,
    )
    .unwrap();
    let data: ChartData = serde_json::from_str(
        r#"{"entities":{"icons":[{"id":"a","type":"P","attributes":{"City":"London"}},
            {"id":"b","type":"P","attributes":{"City":"Paris"}},
            {"id":"c","type":"P","attributes":{"City":"Berlin"}}]},"links":[]}"#,
    )
    .unwrap();
    let xml = Builder::new(&config).build(&data);
    // Equirectangular projection matches the Python reference exactly.
    assert!(xml.contains("<End X=\"166\" Y=\"314\" Z=\"0\">")); // London
    assert!(xml.contains("<End X=\"1833\" Y=\"83\" Z=\"0\">")); // Berlin
}

#[test]
fn display_label_renders_template() {
    let config: Config = serde_json::from_str(
        r#"{"settings":{"extra_cfg":{"display_label":[
            {"key":"k1","template":"{Name} ({Age})","sources":[{"attribute":"Name"},{"attribute":"Age"}]}]}}}"#,
    )
    .unwrap();
    let data: ChartData = serde_json::from_str(
        r#"{"entities":{"icons":[{"id":"a","type":"P","attributes":{"Name":"Alice","Age":30}}]},"links":[]}"#,
    )
    .unwrap();
    let xml = Builder::new(&config).build(&data);
    assert!(xml.contains("Label=\"Alice (30)\""));
}

#[test]
fn pretty_mode_indents_two_spaces_like_python() {
    let data: ChartData =
        serde_json::from_str(r#"{"entities":{"icons":[{"id":"a","type":"P"}]},"links":[]}"#)
            .unwrap();
    let pretty = Builder::new(&Config::default()).build_with(&data, false);
    // Children of <Chart> are indented two spaces (matches to_xml(compact=False)).
    assert!(pretty.contains("\n  <ApplicationVersion"));
    assert!(pretty.contains("\n    <Strength ")); // depth-2
                                                  // Compact build has no indentation.
    let compact = Builder::new(&Config::default()).build(&data);
    assert!(compact.contains("\n<ApplicationVersion"));
    assert!(!compact.contains("\n  <ApplicationVersion"));
}

#[test]
fn custom_icon_embeds_valid_bmp() {
    use base64::Engine;
    use std::io::Read;
    // 1x1 PNG data URI.
    let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
    let config: Config = serde_json::from_str(&format!(
        r#"{{"custom_entity_icons":[{{"name":"vip","image":"{png}"}}],
            "entity_types":[{{"name":"Person","icon_file":"vip"}}]}}"#
    ))
    .unwrap();
    let data: ChartData =
        serde_json::from_str(r#"{"entities":{"icons":[{"id":"a","type":"Person"}]},"links":[]}"#)
            .unwrap();
    let xml = Builder::new(&config).build(&data);
    assert!(xml.contains("<CustomImage Id=\"anxW_vip,Screen,Icon\""));
    assert!(xml.contains("IconFile=\"anxW_vip\""));
    // The Data must decode (zlib) to a valid BMP.
    let data_b64 = xml
        .split("Data=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let compressed = base64::engine::general_purpose::STANDARD
        .decode(data_b64)
        .unwrap();
    let mut d = flate2::read::ZlibDecoder::new(&compressed[..]);
    let mut bmp = Vec::new();
    d.read_to_end(&mut bmp).unwrap();
    assert_eq!(&bmp[..2], b"BM");
}

/// Walk the XML with a real parser to prove well-formedness and count elements.
fn count_tag(xml: &str, tag: &str) -> usize {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;
    let mut reader = Reader::from_str(xml);
    let mut count = 0;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).expect("well-formed XML") {
            Event::Start(e) | Event::Empty(e) => {
                if e.name().as_ref() == tag.as_bytes() {
                    count += 1;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    count
}

#[test]
fn full_example_is_well_formed_with_expected_structure() {
    let config: Config =
        serde_json::from_str(include_str!("fixtures/example_config.json")).unwrap();
    let data: ChartData = serde_json::from_str(include_str!("fixtures/example_data.json")).unwrap();
    let xml = Builder::new(&config).build(&data);

    // 8 entities + 8 links = 16 chart items.
    assert_eq!(count_tag(&xml, "ChartItem"), 16);
    // The 5 configured entity types and 5 link types are present.
    assert_eq!(count_tag(&xml, "EntityType"), 5);
    assert_eq!(count_tag(&xml, "LinkType"), 5);
    // 6 configured attribute classes.
    assert_eq!(count_tag(&xml, "AttributeClass"), 6);
    assert!(xml.contains("<Chart"));
    assert!(xml.contains("Operation Northstar")); // summary title

    // Transform outputs that must match the Python reference:
    // HSV auto-colour on the first entity (idx 0 of 8 -> COLORREF 6776805).
    assert!(xml.contains("IconShadingColour=\"6776805\""));
    // Auto-colour also drives a per-entity CIStyle label font.
    assert_eq!(count_tag(&xml, "CIStyle"), 8);
    // Grade defaults applied to entities/links (Usually reliable = index 1).
    assert!(xml.contains("GradeOneIndex=\"1\""));
    // Datetime parsing on a dated link.
    assert!(xml.contains("DateTime=\"2026-02-14T09:30:00.000\""));
    // link_match_entity_color colours a link from its target entity.
    assert!(xml.contains("<LinkStyle") && xml.contains("LineColour="));
    // Legend emitted from config.
    assert_eq!(count_tag(&xml, "LegendItem"), 3);
}
