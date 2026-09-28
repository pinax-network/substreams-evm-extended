//! Solidity source-map entries are instruction-indexed, not byte-indexed.
//! File IDs are resolved across every input and generated source, never assumed 0.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: i64,
    pub length: i64,
    pub file: i64,
    pub jump: String,
    pub depth: i64,
}
pub fn decode(code: &[u8], map: &str) -> Result<BTreeMap<usize, Span>> {
    let mut result = BTreeMap::new();
    let mut last = Span {
        start: -1,
        length: -1,
        file: -1,
        jump: String::new(),
        depth: 0,
    };
    let mut pc = 0;
    for entry in map.split(';') {
        ensure!(pc < code.len(), "source map exceeds instructions");
        let fields: Vec<_> = entry.split(':').collect();
        ensure!(fields.len() <= 5, "source map field count");
        for (i, v) in fields.iter().enumerate().filter(|(_, v)| !v.is_empty()) {
            match i {
                0 => last.start = v.parse()?,
                1 => last.length = v.parse()?,
                2 => last.file = v.parse()?,
                3 => last.jump = (*v).into(),
                4 => last.depth = v.parse()?,
                _ => unreachable!(),
            }
        }
        ensure!(
            last.file >= -1 && last.start >= -1 && last.length >= -1 && last.depth >= 0,
            "invalid source span"
        );
        result.insert(pc, last.clone());
        let op = code[pc];
        pc += 1 + if (0x60..=0x7f).contains(&op) { usize::from(op - 0x5f) } else { 0 };
    }
    Ok(result)
}
pub fn sources(capture: &Value, compiled: &Value, kind: &str) -> Result<BTreeMap<i64, (String, String)>> {
    let mut result = BTreeMap::new();
    for (path, info) in compiled["sources"].as_object().context("source ids")? {
        let id = info["id"].as_i64().context("source id")?;
        let content = capture["sources"][path]["content"].as_str().context("source content")?;
        ensure!(result.insert(id, (path.clone(), content.into())).is_none(), "duplicate source id");
    }
    for source in compiled["contracts"][super::SOURCE][super::NAME]["evm"][kind]["generatedSources"]
        .as_array()
        .context("generated sources")?
    {
        let id = source["id"].as_i64().context("generated source id")?;
        ensure!(
            result
                .insert(
                    id,
                    (
                        source["name"].as_str().context("generated name")?.into(),
                        source["contents"].as_str().context("generated content")?.into()
                    )
                )
                .is_none(),
            "duplicate generated id"
        );
    }
    Ok(result)
}
pub fn describe(pc: usize, map: &BTreeMap<usize, Span>, sources: &BTreeMap<i64, (String, String)>) -> Result<Value> {
    let Some(span) = map.get(&pc) else {
        return Ok(json!({"pc":pc,"source":"unmapped instruction"}));
    };
    if span.file == -1 {
        return Ok(
            json!({"pc":pc,"file_id":-1,"source":"compiler-generated span without source ID","start":span.start,"length":span.length,"jump":span.jump,"modifier_depth":span.depth}),
        );
    }
    let (path, content) = sources.get(&span.file).context("unresolved source id")?;
    let start = usize::try_from(span.start)?;
    let length = usize::try_from(span.length)?;
    let end = start.checked_add(length).context("source range overflow")?;
    let text = content.get(start..end).context("invalid UTF-8 source byte range")?;
    Ok(
        json!({"pc":pc,"file_id":span.file,"path":path,"start":start,"length":length,"line":1+content[..start].bytes().filter(|b|*b==b'\n').count(),"text":text,"source_sha256":super::sha(content.as_bytes()),"jump":span.jump,"modifier_depth":span.depth}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ptoken_source_map_tracks_push_bytes_compression_and_multiple_files() {
        let code = hex::decode("61ffff5b5f555f").unwrap();
        let map = decode(&code, "0:3:20:i:0;:2:7:o;4::20:-;::7::1").unwrap();
        assert_eq!(map.keys().copied().collect::<Vec<_>>(), vec![0, 3, 4, 5]);
        let sources = BTreeMap::from([(20, ("token".into(), "abc def".into())), (7, ("library".into(), "01234567".into()))]);
        assert_eq!(describe(4, &map, &sources).unwrap()["text"], "de");
        assert_eq!(describe(5, &map, &sources).unwrap()["path"], "library");
        assert_eq!(describe(5, &map, &sources).unwrap()["modifier_depth"], 1);
        assert!(decode(&[0], "0:2:1;0:1:1").is_err());
        assert!(describe(0, &map, &BTreeMap::new()).is_err());
        assert!(decode(&[0], "0:2:-2").is_err());
    }
}
