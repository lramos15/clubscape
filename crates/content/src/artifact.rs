use std::io::{self, Write};

use clubscape_game_types::{GameContent, GameResult};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{CompiledContent, MAX_INPUT_BYTES, ValidationMode, compile_content, decode, invalid};

pub const ARTIFACT_VERSION: u16 = 4;
pub const ARTIFACT_HEADER_BYTES: usize = 84;
const MAGIC: &[u8; 8] = b"CLSCONT\0";
const CODEC_MESSAGEPACK_NAMED: u16 = 1;

/// Lowercase SHA-256, suitable for identifying the entire output file.
pub fn sha256(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(result, "{byte:02x}");
    }
    result
}

pub fn encode_compiled(content: &CompiledContent) -> GameResult<Vec<u8>> {
    let payload = encode_definition(content.definition())?;
    let mut artifact = Vec::with_capacity(ARTIFACT_HEADER_BYTES + payload.len());
    artifact.extend_from_slice(MAGIC);
    artifact.extend_from_slice(&ARTIFACT_VERSION.to_le_bytes());
    artifact.extend_from_slice(&CODEC_MESSAGEPACK_NAMED.to_le_bytes());
    artifact.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    artifact.extend_from_slice(&Sha256::digest(&payload));
    artifact.extend_from_slice(&identity_digest(content.definition()));
    artifact.extend_from_slice(&payload);
    Ok(artifact)
}

/// Loads one exact envelope, then decodes and recompiles its definitions under
/// the caller's validation policy. No indexes or validation claims are trusted.
pub fn load_compiled(input: &[u8], mode: ValidationMode) -> GameResult<CompiledContent> {
    decode::check_size(input)?;
    if input.len() < ARTIFACT_HEADER_BYTES {
        return Err(invalid("artifact.header", "truncated header"));
    }
    if &input[..8] != MAGIC {
        return Err(invalid("artifact.header", "invalid magic"));
    }
    if input[8..10] != ARTIFACT_VERSION.to_le_bytes() {
        return Err(invalid("artifact.header", "unsupported artifact version"));
    }
    if input[10..12] != CODEC_MESSAGEPACK_NAMED.to_le_bytes() {
        return Err(invalid("artifact.header", "unsupported binary codec"));
    }
    let mut size_bytes = [0; 8];
    size_bytes.copy_from_slice(&input[12..20]);
    let size = u64::from_le_bytes(size_bytes);
    let actual = input.len() - ARTIFACT_HEADER_BYTES;
    if size > (MAX_INPUT_BYTES - ARTIFACT_HEADER_BYTES) as u64 {
        return Err(invalid(
            "artifact.header",
            "declared payload exceeds the size limit",
        ));
    }
    if size != actual as u64 {
        return Err(invalid(
            "artifact.header",
            "payload size mismatch or trailing bytes",
        ));
    }
    let payload = &input[ARTIFACT_HEADER_BYTES..];
    if Sha256::digest(payload).as_slice() != &input[20..52] {
        return Err(invalid("artifact.payload", "SHA-256 checksum mismatch"));
    }
    let mut decoder = rmp_serde::Deserializer::from_read_ref(payload);
    let value =
        decode::read_value(&mut decoder).map_err(|error| invalid("artifact.payload", error))?;
    let definition = decode::into_content(value)?;
    if encode_definition(&definition)? != payload {
        return Err(invalid(
            "artifact.payload",
            "noncanonical or trailing MessagePack bytes",
        ));
    }
    if identity_digest(&definition).as_slice() != &input[52..84] {
        return Err(invalid(
            "artifact.identity",
            "schema/revision/baseline digest mismatch",
        ));
    }
    compile_content(definition, mode)
}

fn encode_definition(content: &GameContent) -> GameResult<Vec<u8>> {
    let mut writer = BoundedWriter(Vec::new());
    // JSON-normalized keys also cover typed integer-keyed morph/level tables.
    serde_json::to_value(content)
        .map_err(|error| invalid("artifact.payload", error))?
        .serialize(&mut rmp_serde::Serializer::new(&mut writer).with_struct_map())
        .map_err(|error| invalid("artifact.payload", error))?;
    Ok(writer.0)
}

struct BoundedWriter(Vec<u8>);

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > (MAX_INPUT_BYTES - ARTIFACT_HEADER_BYTES).saturating_sub(self.0.len()) {
            return Err(io::Error::other("compiled content exceeds the size limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn identity_digest(content: &GameContent) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"clubscape.content.identity.v4\0");
    hash.update(content.schema_version.to_le_bytes());
    for field in [&content.revision, &content.baseline] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    hash.finalize().into()
}
