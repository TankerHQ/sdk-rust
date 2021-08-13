use blake2::digest::{Update, VariableOutput};
use blake2::VarBlake2b;
use ed25519_dalek::Keypair;

pub fn serialized_root_block(sign_keypair: &Keypair) -> Vec<u8> {
    const NATURE_TRUSTCHAIN_CREATION: u64 = 1;

    let payload = serialized_trustchain_creation(sign_keypair);
    let author = [0u8; crate::identity::BLOCK_HASH_SIZE];
    let signature = [0u8; crate::identity::SIGNATURE_SIZE];
    let hash = block_hash(NATURE_TRUSTCHAIN_CREATION, &author, &payload);

    serialized_block(
        NATURE_TRUSTCHAIN_CREATION,
        0,
        &hash,
        &payload,
        &author,
        &signature,
    )
}

fn serialized_varint(int: u64) -> impl IntoIterator<Item = u8> {
    assert!(int < 0x80); // 7-bit varints are easy to encode!
    std::iter::once(int as u8)
}

fn serialized_block(
    nature: u64,
    index: u64,
    trustchain_id: &[u8],
    payload: &[u8],
    author: &[u8; 32],
    signature: &[u8],
) -> Vec<u8> {
    const BLOCK_FORMAT_VERSION: u64 = 1;

    let mut data = Vec::new();
    data.extend(serialized_varint(BLOCK_FORMAT_VERSION));
    data.extend(serialized_varint(index));
    data.extend_from_slice(trustchain_id);

    data.extend(serialized_varint(nature));
    data.extend(serialized_varint(payload.len() as u64));
    data.extend_from_slice(payload);

    data.extend_from_slice(author);
    data.extend_from_slice(signature);

    data
}

fn block_hash(nature: u64, author: &[u8; 32], payload: &[u8]) -> Vec<u8> {
    let mut hasher = VarBlake2b::new(crate::identity::BLOCK_HASH_SIZE).unwrap();
    hasher.update(serialized_varint(nature).into_iter().collect::<Vec<_>>());
    hasher.update(author);
    hasher.update(payload);
    hasher.finalize_boxed().to_vec()
}

fn serialized_trustchain_creation(sign_keypair: &Keypair) -> Vec<u8> {
    sign_keypair.public.as_bytes().to_vec()
}
