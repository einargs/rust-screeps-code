//! Module with inline submodules for use with #[cbor(with = "<path>")].

use std::collections::HashMap;
use screeps::local::ObjectId;
use minicbor::{Encode, Decode, Encoder, Decoder};
use minicbor::encode::{Write};
use minicbor::encode;
use minicbor::decode;

pub mod object_id {
  use super::*;
  pub fn decode<'b, Ctx, T>(
    d: &mut Decoder<'b>, ctx: &mut Ctx
  ) -> Result<ObjectId<T>, decode::Error> {
    let raw: [u8; 16] = d.bytes()
      .map_err(|e| e.with_message("error loading ObjectId bytes"))?
      .try_into()
      .or(Err(decode::Error::message("could not decode u128")))?;
    let packed = u128::from_be_bytes(raw);
    Ok(ObjectId::<T>::from_packed(packed).into())
  }

  pub fn encode<Ctx, T, W: Write>(
    v: &ObjectId<T>, e: &mut Encoder<W>, ctx: &mut Ctx
  ) -> Result<(), encode::Error<W::Error>> {
    let packed = ObjectId::<T>::from(*v).to_u128();
    e.bytes(&packed.to_be_bytes())?;
    Ok(())
  }
}

pub mod object_id_map {
  use super::*;
  pub fn decode<'b, Ctx, T, M: 'b>(
    d: &mut Decoder<'b>, ctx: &mut Ctx
  ) -> Result<HashMap<ObjectId<T>, M>, decode::Error> where M: Decode<'b, Ctx> {
    let size = d.array()?
      .ok_or(decode::Error::message("object id map did not have set length"))?;
    let mut map: HashMap<ObjectId<T>, M> = HashMap::with_capacity(size as usize);
    for _ in 0..size {
      let len = d.array()?;
      if len != Some(2) {
        return Err(decode::Error::message("member pair for object_id_map was not an array of two members"));
      }
      let id = object_id::decode(d, ctx)?;
      let mem = M::decode(d, ctx)?;
      map.insert(id, mem);
    }
    Ok(map)
  }

  pub fn encode<Ctx, T, M: Encode<Ctx>, W: encode::Write>(
    map: &HashMap<ObjectId<T>, M>, e: &mut Encoder<W>, ctx: &mut Ctx
  ) -> Result<(), encode::Error<W::Error>> {
    e.array(map.len() as u64)?;
    for (id, mem) in map {
      e.array(2)?;
      object_id::encode(id, e, ctx)?;
      mem.encode(e, ctx)?;
    }
    Ok(())
  }
}
