use std::mem;

use nom::bytes::streaming::{take, take_till};
use serde::Deserialize;
// This should implement a server list ping,
// to error out, if the server doesn't start up.

const VARINT_SEGMENT_VALUE_MASK: u8 = !( 1 << 7);
const VARINT_CONTINUE_BIT: u8 = 1 << 7;

fn write_varint(output: &mut Vec<u8>, v: i32) {
	let span = tracing::debug_span!("Serializing varint", v, v_bits=format!("{v:032b}"));
	let _enter = span.enter();

	let mut bits: u32 = unsafe { mem::transmute::<_, u32>(v) };

	if bits == 0 {
		output.push(0);
		return;
	}

	while bits > 0 {
		let value = (bits & VARINT_SEGMENT_VALUE_MASK as u32) as u8;

		bits >>= 7;

		if bits > 0 {
			let byte = value | VARINT_CONTINUE_BIT;
			tracing::debug!(bits=format!("{byte:08b}"), "Serializing non-stop byte");
			output.push(byte);
		} else {
			let byte = value;
			tracing::debug!(bits=format!("{byte:08b}"), "Serializing stop byte");
			output.push(byte);
			break;
		}

	}
}

fn read_varint<'n>(input: &'n [u8]) -> nom::IResult<&'n [u8], i32> {
	let span = tracing::debug_span!("Deserializing varint");
	let _guard = span.enter();

	// All varint bytes except for the last one
	let (input, until_stop) = take_till(|v| v & VARINT_CONTINUE_BIT == 0)(input)?;
	// The final varint byte
	let (input, stop_byte) = take::<_, &[u8], nom::error::Error<&[u8]>>(1_usize)(input)?;

	let len = until_stop.len() + stop_byte.len();

	span.record("len", len);

	assert!(len <= 5, "Varints max out at 5 bytes");

	let mut output: u32 = 0;
	let mut position = 0_usize;

	for i in until_stop.into_iter().chain(stop_byte.into_iter()).copied() {
		let var_value = ((i & VARINT_SEGMENT_VALUE_MASK) as u32) << position;
		output |= var_value;
		tracing::debug!(add=var_value, output, "Adding varint to output");
		position += 7;
	}

	let output_int: i32 = unsafe { mem::transmute(output) };

	nom::IResult::Ok((input, output_int))
}

fn varint_len(v: i32) -> usize {
	// Amount of bits necessary to represent the number.
	// Eg:
	// -1 -> 32 (because of two's complement)
	// 1 -> 1
	// 128 -> 8
	// 16384 -> 15
	let bits = 32 - v.leading_zeros();

	match bits {
		0..=7 => 1,
		8..=14 => 2,
		15..=21 => 3,
		22..=28 => 4,
		29..=32 => 5,
		_ => panic!("Leading zeros should be between 0 and 32"),
	}
}

fn write_short(output: &mut Vec<u8>, v: u16) {
	let span = tracing::debug_span!("serializing short", v);
	let _enter = span.enter();

	let bytes = v.to_be_bytes();
	output.extend(bytes);
	tracing::debug!(?bytes, "serialized short");
}

#[allow(dead_code)]
fn read_short<'n>(input: &'n [u8]) -> nom::IResult<&'n [u8], u16> {
	let span = tracing::debug_span!("read short");
	let _enter = span.enter();

	let (input, short_bytes) = take(2_usize)(input)?;
	tracing::debug!(?short_bytes, "deserializing short");

	let v = u16::from_be_bytes(short_bytes.try_into().unwrap());
	tracing::debug!(v, "deserialized short");

	return nom::IResult::Ok((input, v))
}

fn write_string(output: &mut Vec<u8>, v: &str, max_len: u16) {
	let span = tracing::debug_span!("serializing string", v=?v.as_bytes());
	let _enter = span.enter();

	assert!(v.len() < max_len as usize);

	let len = v.len();
	let data = v.as_bytes();

	write_varint(output, len as i32);
	tracing::debug!("serialized varint");

	output.extend(data);
}

fn read_string<'n>(input: &'n [u8]) -> nom::IResult<&'n [u8], &'n str> {
	let span = tracing::debug_span!("deserializing string");
	let _enter = span.enter();

	let (input, len) = read_varint(input)?;

	tracing::debug!(len, "string length found");

	assert!(len > 0);
	let len = len as usize;

	let (input, data) = take(len)(input)?;

	let string = std::str::from_utf8(data).expect("String contained invalid UTF-8");

	return nom::IResult::Ok((input, string))
}

fn str_len(v: &str) -> usize {
	let len = v.len();
	let length_len = varint_len(len.try_into().expect("len is bigger than i32::MAX"));

	len + length_len
}

fn write_packet(id: i32, data: &[u8]) -> Vec<u8> {
	let span = tracing::debug_span!("serializing packet", id, len=data.len());
	let _enter = span.enter();

	let packet_id_len = varint_len(id);
	let data_len = data.len();

	let packet_length = packet_id_len + data_len;
	tracing::debug!(packet_length, "packet length (no packet length field included)");

	let packet_length_len = varint_len(packet_length.try_into().expect("len is bigger than i32::MAX"));

	let total_packet_length = packet_length_len + packet_length;
	tracing::debug!(total_packet_length, "allocating buffer for packet");
	let mut output = Vec::with_capacity(total_packet_length);

	write_varint(&mut output, packet_length.try_into().expect("Packet length bigger than i32::MAX"));
	write_varint(&mut output, id);
	output.extend(data);

	assert_eq!(output.len() - packet_length_len, packet_length);

	return output
}

fn read_packet<'n>(input: &'n [u8]) -> nom::IResult<&'n [u8], (i32, &'n [u8])> {
	let span = tracing::debug_span!("deserializing packet");
	let _enter = span.enter();

	let (input, packet_length) = read_varint(input)?;

	assert!(packet_length > 0);

	let packet_length = packet_length as usize;
	tracing::debug!(packet_length, "packet length");

	let (input, packet_id) = read_varint(input)?;

	let packet_id_len = varint_len(packet_id);

	let data_len = packet_length - packet_id_len;
	tracing::debug!(data_len, "data length");

	let (input, data) = take(data_len)(input)?;

	return nom::IResult::Ok((input, (packet_id, data)))
}

pub fn server_list_ping(server_host: &str, server_port: u16) -> Vec<u8> {
	let span = tracing::debug_span!("server list ping", server_host, server_port);
	let _enter = span.enter();

	let packet_id = 0x00;
	let protocol_version = 764;
	let next_state = 1;

	// 2 is for the length of a short
	let data_len = varint_len(protocol_version) + str_len(server_host) + 2 + varint_len(next_state);
	tracing::debug!(data_len, "server list ping data size calculated");

	let mut data = Vec::with_capacity(data_len);

	write_varint(&mut data, protocol_version);
	write_string(&mut data, server_host, 32767);
	write_short(&mut data, server_port);
	write_varint(&mut data, next_state);

	write_packet(packet_id, data.as_slice())
}

pub fn status_request() -> Vec<u8> {
	write_packet(0x00, &[])
}

#[allow(dead_code)]
pub fn ping_request() -> Vec<u8> {
	write_packet(0x00, &[])
}

#[derive(thiserror::Error, Debug)]
pub enum ParseError<'n> {
	#[error("Input is incomplete")]
	Incomplete,
	#[error("Error while parsing")]
	Parsing(nom::Err<nom::error::Error<&'n [u8]>>)
}

impl<'n> From<nom::Err<nom::error::Error<&'n [u8]>>> for ParseError<'n> {
	fn from(e: nom::Err<nom::error::Error<&'n [u8]>>) -> Self {
		match e {
			nom::Err::Incomplete(_) => ParseError::Incomplete,
			_ => ParseError::Parsing(e),
		}
	}
}

#[derive(Debug)]
pub struct StatusResponse {
	pub json_response: JsonStatusResponse,
}

#[derive(Deserialize, Debug)]
pub struct JsonStatusResponse {
	pub version: JsonVersion,
	pub players: JsonPlayers,
	pub description: JsonDescription,
	pub favicon: Option<String>,
	#[serde(rename = "enforcesSecureChat", default)]
	pub enforces_secure_chat: bool,
	#[serde(rename = "previewsChat", default)]
	pub previews_chat: bool,
}

#[derive(Deserialize, Debug)]
pub struct JsonVersion {
	pub name: String,
	pub protocol: i32,
}

#[derive(Deserialize, Debug)]
pub struct JsonPlayers {
	pub max: u32,
	pub online: u32,
	pub sample: Option<Vec<JsonPlayer>>,
}

#[derive(Deserialize, Debug)]
pub struct JsonPlayer {
	pub name: String,
	pub id: String,
}

#[derive(Deserialize, Debug)]
pub struct JsonDescription {
	pub text: String,
}

pub fn parse_status_response(buf: &[u8]) -> Result<(usize, StatusResponse), ParseError> {
	let span = tracing::debug_span!("deserializing status response");
	let _enter = span.enter();

	let (input, (packet_id, data)) = read_packet(buf)?;

	let consumed = buf.len() - input.len();

	assert_eq!(packet_id, 0x00);

	let (_, json_response) = read_string(data)?;
	tracing::debug!("got json response (should probably be deserialized)");

	let json_response: JsonStatusResponse = serde_json::from_str(json_response).unwrap();

	return Ok((consumed, StatusResponse { json_response }))
}

#[cfg(test)]
mod test {
    use super::{varint_len, write_varint, read_varint};

	#[test]
	fn test_varint_len() {
		let test_vectors = [
			(1, 1_usize),
			(128, 2_usize),
			(16384, 3_usize),
			(2097152, 4_usize),
			(268435456, 5_usize),
			(-1, 5_usize),
			(-128, 5_usize),
			(-16384, 5_usize),
			(-2097152, 5_usize),
			(-268435456, 5_usize),
		];

		for (v, r) in test_vectors {
			assert_eq!(varint_len(v), r);
		}
	}

	#[test]
	fn test_var_parse_serialize() {
		let test_vectors = [
			764,
			-1,
			80,
			25565,
			1,
		];

		for i in test_vectors {
			let mut output = Vec::with_capacity(10);

			write_varint(&mut output, i);

			let (_, r) = read_varint(&output).unwrap();

			assert_eq!(i, r);
		}
	}
}