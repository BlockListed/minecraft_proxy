use std::mem;

use nom::{bytes::streaming::{take_while_m_n, take, take_until, take_till}, error::ParseError};
// This should implement a server list ping,
// to error out, if the server doesn't start up.

const VARINT_SEGMENT_VALUE_MASK: u8 = !( 1 << 7);
const VARINT_CONTINUE_BIT: u8 = 1 << 7;

fn write_varint(output: &mut Vec<u8>, v: i32) {
	let mut bits: u32 = unsafe { mem::transmute::<_, u32>(v) };

	if bits == 0 {
		output.push(0);
		return;
	}

	println!("Starting serialising");
	while bits > 0 {
		let value = (bits & VARINT_SEGMENT_VALUE_MASK as u32) as u8;

		bits >>= 7;

		if bits > 0 {
			let byte = value | VARINT_CONTINUE_BIT;
			println!("Serialising {byte:08b}");
			output.push(byte);
		} else {
			let byte = value;
			println!("Serialising {byte:08b}");
			output.push(byte);
			break;
		}

	}
	println!("Stopping serialising");
}

fn read_varint<'n>(input: &'n [u8]) -> nom::IResult<&'n [u8], i32> {
	let empty: &[u8] = &[];
	let (input, until_stop) = match take_till(|v| v & VARINT_CONTINUE_BIT == 1)(input) {
		Ok(d) => d,
		Err(e) => match e {
			nom::Err::Incomplete(_) => (input, empty),
			_ => return Err(e),
		}
	};
	let (input, stop_byte) = take::<_, &[u8], ()>(1usize)(input).unwrap();

	assert!(until_stop.len() + stop_byte.len() <= 5, "Varints max out at 5 bytes");

	let mut output: u32 = 0;
	let mut position = 0usize;

	for i in until_stop.into_iter().chain(stop_byte.into_iter()).copied() {
		output |= (i as u32) << position;
		position += 7;
	}

	let output_int: i32 = unsafe { mem::transmute(output) };
	println!("Deserialized {output_int}");

	nom::IResult::Ok((input, output_int))
}

fn varint_len(v: i32) -> usize {
	let bits = 32 - v.leading_zeros();
	match bits {
		0..=7 => 1,
		8..=14 => 2,
		15..=21 => 3,
		22..=28 => 4,
		29..=32 => 5,
		_ => panic!("Leading zeros should b between 0 and 32"),
	}
}

fn write_short(output: &mut Vec<u8>, v: u16) {
	output.extend(v.to_be_bytes())
}

fn read_short<'n>(input: &'n [u8]) -> nom::IResult<&'n [u8], u16> {
	let (input, short_bytes) = take(2usize)(input)?;

	let mut short_bits = [0u8; 2];
	short_bits.copy_from_slice(short_bytes);

	return nom::IResult::Ok((input, u16::from_be_bytes(short_bits)))
}

fn write_string(output: &mut Vec<u8>, v: &str, max_len: u16) {
	assert!(v.len() < max_len as usize);

	let len = v.len();
	let data = v.as_bytes();

	println!("Serialising string {data:?}");

	write_varint(output, len as i32);
	output.extend(data);
}

fn read_string<'n>(input: &'n [u8]) -> nom::IResult<&'n [u8], &'n str> {
	let (input, len) = read_varint(input).unwrap();
	assert!(len > 0);
	let len = len as usize;

	// Add tracing for reading string here
	let (input, data) = take(len)(input)?;

	let string = std::str::from_utf8(data).expect("String contained invalid UTF-8");

	return nom::IResult::Ok((input, string))
}

fn str_len(v: &str) -> usize {
	let len = v.len();
	let length_len = varint_len(len.try_into().unwrap());

	len + length_len
}

fn write_packet(id: i32, data: &[u8]) -> Vec<u8> {
	let packet_id_len = varint_len(id);
	let data_len = data.len();

	let packet_length = packet_id_len + data_len;

	let packet_length_len = varint_len((packet_id_len + data_len).try_into().unwrap());

	let mut output = Vec::with_capacity((packet_length_len + packet_length) as usize);

	write_varint(&mut output, packet_length.try_into().unwrap());
	write_varint(&mut output, id);
	output.extend(data);

	assert_eq!(output.len() - packet_length_len, packet_length);

	return output
}

fn read_packet<'n>(input: &'n [u8]) -> nom::IResult<&'n [u8], (i32, &'n [u8])> {
	let (input, packet_length) = read_varint(input)?;

	assert!(packet_length > 0);

	let packet_length: usize = packet_length as usize;
	println!("Deserializing packet of length {packet_length}");

	let (input, packet_id) = read_varint(input)?;

	let packet_id_len = varint_len(packet_id);

	let data_len = packet_length - packet_id_len as usize;
	println!("Deserializing packet with data of length {data_len}");

	let (input, data) = take(data_len)(input)?;

	return nom::IResult::Ok((input, (packet_id, data)))
}

pub fn server_list_ping(server_host: &str, server_port: u16) -> Vec<u8> {
	let packet_id = 0x00;
	let protocol_version = -1;
	let next_state = 1;

	let data_len = varint_len(protocol_version) + str_len(server_host) + 2 + varint_len(next_state);

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

#[derive(Debug)]
pub struct StatusResponse {
	json_response: String,
}

pub enum Parse<R> {
	Done(usize, R),
	Skip(usize),
	MoreData,
}

pub fn parse_status_response(buf: &[u8]) -> Parse<StatusResponse> {
	let (input, (packet_id, data)) = match read_packet(buf) {
		Ok(d) => d,
		Err(e) => match e {
			nom::Err::Incomplete(_) => {
				println!("Incomplete data packet header {buf:?}");
				return Parse::MoreData;
			},
			_ => Err(e).unwrap(),
		}
	};

	if packet_id != 0x00 {
		println!("Skipping because of wrong packet_id, {packet_id}");
		return Parse::Skip(buf.len() - input.len());
	}

	let (_, json_response) = match read_string(data) {
		Ok(d) => d,
		Err(e) => match e {
			nom::Err::Incomplete(_) => {
				println!("Incomplete data json string {buf:?}");
				return Parse::MoreData;
			},
			_ => Err(e).unwrap(),
		}
	};
	
	let json_response = json_response.to_owned();

	return Parse::Done(buf.len() - input.len(), StatusResponse { json_response })
}

#[cfg(test)]
mod test {
    use super::varint_len;

	#[test]
	fn test_varint_len() {
		let test_vectors = [
			(1, 1usize),
			(128, 2usize),
			(16384, 3usize),
			(2097152, 4usize),
			(268435456, 5usize),
			(-1, 5usize),
			(-128, 5usize),
			(-16384, 5usize),
			(-2097152, 5usize),
			(-268435456, 5usize),
		];

		for (v, r) in test_vectors {
			assert_eq!(varint_len(v), r);
		}
	}
}