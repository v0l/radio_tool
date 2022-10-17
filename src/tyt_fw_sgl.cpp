/**
 * This file is part of radio_tool.
 * Copyright (c) 2022 v0l <radio_tool@v0l.io>
 *
 * radio_tool is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * radio_tool is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with radio_tool. If not, see <https://www.gnu.org/licenses/>.
 */
#include <radio_tool/fw/tyt_fw_sgl.hpp>
#include <radio_tool/util.hpp>

#include <random>
#include <iterator>

using namespace radio_tool::fw;

constexpr auto HeaderLen = 0x400u;
constexpr auto Header1Len = 0x10u;
constexpr auto Header2Len = 0x67u;

constexpr auto BinaryLenOffset = 0x06;
constexpr auto GroupOffset = 0x32;
constexpr auto ModelOffset = GroupOffset + 0x10;
constexpr auto VersionOffset = ModelOffset + 0x08;
constexpr auto KeyOffset = 0x5f;

auto TYTSGLFW::Read(const std::string& file) -> void
{
	auto hdr = ReadHeader(file);

	for (const auto& cfg : tyt::config::sgl::All) {
		if (cfg.header.radio_group == hdr.radio_group) {
			config = new TYTSGLRadioConfig(cfg.radio_model, hdr, cfg.cipher, cfg.cipher_len, cfg.xor_offset);
			break;
		}
	}

	if (config == nullptr) {
		std::stringstream msg;
		msg << "Radio model '" << hdr.radio_group << "' not supported!";

		throw std::runtime_error(msg.str());
	}

	auto i = std::ifstream(file, std::ios_base::binary);
	if (i.is_open())
	{
		i.seekg(0x400, i.beg);
		i.seekg(hdr.binary_offset, i.cur);

		data.resize(hdr.length);
		i.read((char*)data.data(), hdr.length);

		memory_ranges.push_back(std::pair<uint32_t, uint32_t>(0, data.size()));
	}
	i.close();
}

auto TYTSGLFW::Write(const std::string& file) -> void
{
	std::ofstream fout(file, std::ios_base::binary);
	if (fout.is_open())
	{
		auto header = config->header.Serialize();
		fout.write((char*)header.data(), header.size());
		fout.write((char*)data.data(), data.size());
	}
	fout.close();
}

auto TYTSGLFW::ToString() const -> std::string
{
	std::stringstream out;
	out << "== TYT SGL Firmware == " << std::endl
		<< "Radio: " << config->radio_model << std::endl
		<< config->header.ToString() << std::endl
		<< "Data Segments: " << std::endl;
	auto n = 0;
	for (const auto& m : memory_ranges)
	{
		out << "  " << n++ << ": Length=0x" << std::setfill('0') << std::setw(8) << std::hex << m.second
			<< std::endl;
	}
	return out.str();
}

auto TYTSGLFW::ReadHeader(const std::string& file) -> const SGLHeader
{
	std::ifstream i;
	i.open(file, i.binary);
	if (i.is_open())
	{
		uint8_t header1[Header1Len];
		i.read((char*)header1, Header1Len);

		if (!std::equal(header1, header1 + 4, tyt::config::sgl::Magic.begin()))
		{
			throw std::runtime_error("Invalid SGL header magic");
		}

		for (auto x = 4;x < Header1Len;x++) {
			header1[x] ^= tyt::config::sgl::Magic[x % 4];
		}

		auto sgl_version = std::stoi((char*)header1 + 9);
		if (sgl_version != 1) {
			std::stringstream msg("Invalid SGL version: ");
			msg << sgl_version;
			throw std::runtime_error(msg.str());
		}

		auto header2_offset = *(uint16_t*)(header1 + 12);

		i.seekg(header2_offset, i.beg);
		uint8_t header2[Header2Len];
		i.read((char*)header2, Header2Len);

		auto h2_xor = header1 + 14;
		for (auto x = 0;x < Header2Len;x++) {
			header2[x] ^= h2_xor[x % 2];
		}

		auto binary_offset = header1[11];
		auto len = *(uint32_t*)(header2 + BinaryLenOffset);
		auto group = std::string(header2 + GroupOffset, header2 + GroupOffset + 0x10);
		auto model = std::string(header2 + ModelOffset, header2 + ModelOffset + 0x08);
		auto version = std::string(header2 + VersionOffset, header2 + VersionOffset + 0x08);
		auto key = std::string(header2 + KeyOffset, header2 + KeyOffset + 0x08);

		return SGLHeader(sgl_version, len, group, model, version, key, binary_offset, header2_offset);
	}
	else
	{
		throw std::runtime_error("Can't open firmware file");
	}
}

auto TYTSGLFW::SupportsFirmwareFile(const std::string& file) -> bool
{
	auto header = ReadHeader(file);
	if (header.length != 0)
	{
		return true;
	}
	return false;
}

auto TYTSGLFW::SupportsRadioModel(const std::string& model) -> bool
{
	for (const auto& mx : tyt::config::sgl::All)
	{
		if (mx.radio_model == model)
		{
			return true;
		}
	}
	return false;
}

auto TYTSGLFW::GetRadioModel() const -> const std::string
{
	auto ret = std::string(config->radio_model);

	auto end = std::find_if(ret.begin(), ret.end(), [](unsigned char ch) {
		return ch == '\xff';
		});
	return ret.substr(0, end - ret.begin());
}

auto TYTSGLFW::SetRadioModel(const std::string& model) -> void
{
	for (const auto& rg : tyt::config::sgl::All)
	{
		if (rg.radio_model == model)
		{
			config = &rg;
			break;
		}
	}
}

auto TYTSGLFW::Decrypt() -> void
{
	auto cx = 0;
	for (auto& dx : data)
	{
		dx = ~(((dx << 3) & 0b11111000) | ((dx >> 5) & 0b00000111));
		dx = dx ^ config->cipher[(config->xor_offset + cx++) % config->cipher_len];
	}
}

auto TYTSGLFW::Encrypt() -> void
{
	// before encrypting make sure the data is the correct length
	// if too short add padding
	// if too big? ...official firmware writing tool wont work probably
	// padding is normally handled by FirmwareSupport::AppendSegment
	if (data.size() < config->header.length)
	{
		std::fill_n(std::back_inserter(data), config->header.length - data.size(), 0xff);
	}

	auto cx = 0;
	for (auto& dx : data)
	{
		dx = dx ^ config->cipher[(config->xor_offset + cx++) % config->cipher_len];
		dx = ~(((dx >> 3) & 0b00011111) | ((dx << 5) & 0b11100000));
	}
}

auto SGLHeader::ToString() const -> std::string
{
	std::stringstream ss;

	ss << "SGL version: " << sgl_version << std::endl
		<< "Length: " << length << std::endl
		<< "Radio Group: " << radio_group << std::endl
		<< "Model: " << radio_model << std::endl
		<< "Protocol Version: " << protocol_version << std::endl
		<< "Key: " << model_key << std::endl
		<< "Binary Offset: 0x400 + 0x" << std::hex << std::setw(2) << std::setfill('0') << (int)binary_offset;

	return ss.str();
}

auto SGLHeader::Serialize(bool encrypt) const -> std::vector<uint8_t>
{
	auto header = std::vector<uint8_t>(HeaderLen + binary_offset);

	// SGL!
	std::copy(tyt::config::sgl::Magic.begin(), tyt::config::sgl::Magic.end(), header.begin());

	// ENV001
	std::stringstream ss_version;
	ss_version << "ENCV" << std::setw(3) << std::setfill('0') << sgl_version;
	auto version = ss_version.str();
	std::copy(version.begin(), version.end(), header.begin() + 4);

	// binary_offset
	*(header.data() + 11) = binary_offset;

	// h2 offset
	*(uint16_t*)(header.data() + 12) = header2_offset;

	// h2 xor key
	auto h2_key_offset = 14;
	std::default_random_engine eng;
	std::uniform_int_distribution<uint16_t> dist(0, 0xffff);
	auto key = dist(eng);
	*(uint16_t*)(header.data() + h2_key_offset) = key;

	auto h2 = header.data() + Header1Len;
	*h2 = 0x02;
	*(h2 + 1) = 0x10;

	// binary length
	*(uint32_t*)(h2 + BinaryLenOffset) = length;

	std::copy(radio_group.begin(), radio_group.end(), h2 + GroupOffset);
	std::copy(radio_model.begin(), radio_model.end(), h2 + ModelOffset);
	std::copy(protocol_version.begin(), protocol_version.end(), h2 + VersionOffset);
	std::copy(model_key.begin(), model_key.end(), h2 + KeyOffset);

	if (encrypt) {
		// encrypt header 2
		auto h2_key = header.data() + h2_key_offset;
		for (auto h2x = 0;h2x < Header2Len;h2x++) {
			*(h2 + h2x) ^= *(h2_key + (h2x % 2));
		}

		// encrypt header 1
		for (auto h1x = 4;h1x < Header1Len;h1x++) {
			*(header.data() + h1x) ^= tyt::config::sgl::Magic[h1x % 4];
		}
	}

	return header;
}