/**
 * This file is part of radio_tool.
 * Copyright (c) 2026 v0l <radio_tool@v0l.io>
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
#include <radio_tool/radio/uv17pro_radio.hpp>
#include <radio_tool/device/serial_port.hpp>

#include <fstream>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <cstring>
#include <algorithm>

using namespace radio_tool::radio;
using namespace radio_tool::device;

namespace
{
	constexpr auto ACK = 0x06;
	//marks a codeplug saved by CHIRP, which appends metadata after the image
	constexpr uint8_t ChirpMagic[] = {'c', 'h', 'i', 'r', 'p', 0xee, 'i', 'm', 'g'};
	//a BLE link is much slower to answer than a USB cable, the first reply
	//after connecting has been measured at well over a second
	constexpr auto ReadTimeout = 8000u;

	//sent after the ident magic, the responses are not checked, only consumed
	const std::vector<std::pair<std::vector<uint8_t>, size_t>> HandshakeMagics = {
		{{0x46}, 16},
		{{0x4d}, 15},
		{{0x53, 0x45, 0x4E, 0x44, 0x21, 0x05, 0x0D, 0x01, 0x01, 0x01, 0x04, 0x11, 0x08,
		  0x05, 0x0D, 0x0D, 0x01, 0x11, 0x0F, 0x09, 0x12, 0x09, 0x10, 0x04, 0x00},
		 1}};

	auto ToBytes(const char *str) -> std::vector<uint8_t>
	{
		return std::vector<uint8_t>(str, str + strlen(str));
	}
}

auto UV17ProRadio::Crypt(const uint8_t &key_index, const std::vector<uint8_t> &data) -> std::vector<uint8_t>
{
	//table lifted from CHIRP, only "CO 7" is used by the radios we support
	static const char *table[] = {
		"BHT ", "CO 7", "A ES", " EIY", "M PQ",
		"XN Y", "RVB ", " HQP", "W RC", "MS N",
		" SAT", "K DH", "ZO R", "C SL", "6RB ",
		" JCG", "PN V", "J PK", "EK L", "I LZ"};

	if (key_index >= (sizeof(table) / sizeof(table[0])))
	{
		throw std::runtime_error("Invalid encryption key index");
	}
	const auto *key = table[key_index];

	std::vector<uint8_t> out;
	out.reserve(data.size());

	auto key_ix = 0;
	for (size_t ix = 0; ix < data.size(); ix++)
	{
		auto k = (uint8_t)key[key_ix];
		auto b = data[ix];

		//spaces in the key, and the trivial byte values, are passed through
		auto encrypt = (k != ' ') && (b != 0x00) && (b != 0xff) && (b != k) && (b != (uint8_t)(k ^ 0xff));
		out.push_back(encrypt ? (uint8_t)(b ^ k) : b);

		key_ix = (key_ix + 1) % 4;
	}
	return out;
}

auto UV17ProRadio::Models() -> const std::vector<UV17ProModel> &
{
	//magic strings and memory maps from chirp/drivers/baofeng_uv17Pro.py
	static const std::vector<UV17ProModel> models = {
		{"UV5RMINI", "PROGRAMCOLORPROU", "UV-5R Mini",
		 {{0x0000, 0x8040}, {0x9000, 0x0040}, {0xA000, 0x01C0}},
		 1},
		{"UV5GMINI", "PROGRAMCOLORPROU", "UV-5G Mini",
		 {{0x0000, 0x8040}, {0x9000, 0x0040}, {0xA000, 0x01C0}},
		 1},
		{"UV17PRO", "PROGRAMBFNORMALU", "UV-17Pro",
		 {{0x0000, 0x8040}, {0x9000, 0x0040}, {0xA000, 0x02C0}, {0xD000, 0x0040}},
		 1},
		{"UV17PROGPS", "PROGRAMCOLORPROU", "UV-17ProGPS",
		 {{0x0000, 0x8040}, {0x9000, 0x0040}, {0xA000, 0x02C0}, {0xD000, 0x0040}},
		 1},
	};
	return models;
}

auto UV17ProRadio::GetModel(const std::string &name) -> const UV17ProModel *
{
	for (const auto &m : Models())
	{
		if (name == m.name)
		{
			return &m;
		}
	}
	return nullptr;
}

UV17ProRadio::UV17ProRadio(const std::string &prt, const UV17ProModel &model)
	: port(std::make_unique<SerialPort>(prt, BaudRate)), model(model)
{
}

UV17ProRadio::UV17ProRadio(std::unique_ptr<ByteStream> stream, const UV17ProModel &model)
	: port(std::move(stream)), model(model)
{
}

auto UV17ProRadio::WriteFirmware(const std::string &) -> void
{
	throw std::runtime_error("Firmware upgrade is not supported for this radio");
}

auto UV17ProRadio::SendMagic(const std::vector<uint8_t> &magic, const size_t &response_len, const std::string &what) const -> std::vector<uint8_t>
{
	port->Write(magic);
	return port->ReadExact(response_len, ReadTimeout, what);
}

auto UV17ProRadio::Identify() const -> void
{
	if (!ident_response.empty())
	{
		return;
	}

	port->FlushInput();

	auto ack = SendMagic(ToBytes(model.ident), 1, "ident response");
	if (ack[0] != ACK)
	{
		std::stringstream err;
		err << "Radio did not accept the ident magic for " << model.name
			<< " (got 0x" << std::hex << (int)ack[0] << ", expected 0x06)";
		throw std::runtime_error(err.str());
	}

	//these responses carry the model and firmware strings, the radio wants
	//them read before it will accept memory reads
	auto ix = 0;
	for (const auto &m : HandshakeMagics)
	{
		std::stringstream what;
		what << "handshake response " << ++ix;
		auto rsp = SendMagic(m.first, m.second, what.str());
		if (ident_response.empty())
		{
			ident_response = rsp;
		}
	}
}

auto UV17ProRadio::ReadBlock(const uint16_t &addr, const uint8_t &size) const -> std::vector<uint8_t>
{
	std::vector<uint8_t> frame = {(uint8_t)'R', (uint8_t)(addr >> 8), (uint8_t)(addr & 0xff), size};
	port->Write(frame);

	std::stringstream what;
	what << "block 0x" << std::hex << std::setw(4) << std::setfill('0') << addr;
	auto rsp = port->ReadExact(size + 4, ReadTimeout, what.str());

	//the header is echoed back with the command byte the radio feels like
	//using, CHIRP ignores it entirely, we only check we are still in step
	auto r_addr = (uint16_t)((rsp[1] << 8) | rsp[2]);
	if (r_addr != addr || rsp[3] != size)
	{
		std::stringstream err;
		err << "Out of step reading block 0x" << std::hex << std::setw(4) << std::setfill('0') << addr
			<< " (got addr=0x" << r_addr << " size=0x" << (int)rsp[3] << ")";
		throw std::runtime_error(err.str());
	}

	return Crypt(model.encryption_key, std::vector<uint8_t>(rsp.begin() + 4, rsp.end()));
}

auto UV17ProRadio::MemoryTotal() const -> size_t
{
	size_t total = 0;
	for (const auto &r : model.regions)
	{
		total += r.size;
	}
	return total;
}

auto UV17ProRadio::Download() const -> std::vector<uint8_t>
{
	Identify();

	auto total = MemoryTotal();

	std::vector<uint8_t> data;
	data.reserve(total);

	std::cerr << "Downloading codeplug..." << std::endl;
	for (const auto &region : model.regions)
	{
		for (auto addr = region.start; addr < region.start + region.size; addr += BlockSize)
		{
			auto block = ReadBlock((uint16_t)addr, BlockSize);
			data.insert(data.end(), block.begin(), block.end());

			std::cerr << "\r 0x" << std::hex << std::setw(4) << std::setfill('0') << (int)addr
					  << " (" << std::dec << data.size() << " / " << total << " bytes)" << std::flush;
		}
	}
	std::cerr << std::endl;

	return data;
}

auto UV17ProRadio::WriteBlock(const uint16_t &addr, const std::vector<uint8_t> &data) const -> void
{
	std::vector<uint8_t> frame = {(uint8_t)'W', (uint8_t)(addr >> 8), (uint8_t)(addr & 0xff), (uint8_t)data.size()};
	auto payload = Crypt(model.encryption_key, data);
	frame.insert(frame.end(), payload.begin(), payload.end());

	port->Write(frame);

	std::stringstream what;
	what << "ack for block 0x" << std::hex << std::setw(4) << std::setfill('0') << addr;
	auto ack = port->ReadExact(1, ReadTimeout, what.str());

	if (ack[0] != ACK)
	{
		std::stringstream err;
		err << "Radio rejected block 0x" << std::hex << std::setw(4) << std::setfill('0') << addr
			<< " (got 0x" << (int)ack[0] << ", expected 0x06)";
		throw std::runtime_error(err.str());
	}
}

auto UV17ProRadio::Upload(const std::vector<uint8_t> &data) const -> void
{
	auto total = MemoryTotal();
	if (data.size() < total)
	{
		std::stringstream err;
		err << "Codeplug is too small for this radio (have " << std::dec << data.size()
			<< " bytes, need " << total << ")";
		throw std::runtime_error(err.str());
	}

	Identify();

	//a cable carries the memory in small blocks, BLE in larger ones, matching
	//what the vendor software and CHIRP do on each link
	auto block_size = port->BlockSizeHint();
	if (block_size == 0 || block_size > 0xff)
	{
		throw std::runtime_error("Invalid block size for this link");
	}

	size_t done = 0;
	std::cerr << "Uploading codeplug..." << std::endl;
	for (const auto &region : model.regions)
	{
		for (auto addr = region.start; addr < region.start + region.size; addr += block_size)
		{
			//the last block of a region can be short. The frame carries its own
			//length, so send a short block rather than padding out past the end
			//of the region and writing memory the region does not cover
			auto count = std::min((size_t)block_size, (size_t)(region.start + region.size - addr));
			std::vector<uint8_t> block(data.begin() + done, data.begin() + done + count);
			done += count;

			WriteBlock((uint16_t)addr, block);

			std::cerr << "\r 0x" << std::hex << std::setw(4) << std::setfill('0') << (int)addr
					  << " (" << std::dec << done << " / " << total << " bytes)" << std::flush;
		}
	}
	std::cerr << std::endl;
}

auto UV17ProRadio::WriteCodeplug(const std::string &file) -> void
{
	std::ifstream in(file, std::ios_base::in | std::ios_base::binary | std::ios_base::ate);
	if (!in.is_open())
	{
		throw std::runtime_error("Cant open file: " + file);
	}

	auto size = (size_t)in.tellg();
	in.seekg(0);
	std::vector<uint8_t> data(size);
	in.read((char *)data.data(), size);
	in.close();

	auto total = MemoryTotal();
	if (size < total)
	{
		std::stringstream err;
		err << "Codeplug " << file << " is too small for a " << model.name
			<< " (" << std::dec << size << " bytes, expected at least " << total << ")";
		throw std::runtime_error(err.str());
	}

	//anything past the memory itself is the model name we stamp on a download,
	//or metadata added by CHIRP, and is not sent to the radio
	auto tail = std::vector<uint8_t>(data.begin() + total, data.end());
	auto expect = ToBytes(model.chirp_model);
	auto tagged = tail.size() >= expect.size() &&
				  std::equal(expect.begin(), expect.end(), tail.begin());
	auto from_chirp = std::search(tail.begin(), tail.end(), ChirpMagic, ChirpMagic + sizeof(ChirpMagic)) != tail.end();

	if (!tail.empty() && !tagged && !from_chirp)
	{
		std::cerr << "Warning: " << file << " is not stamped for a " << model.chirp_model
				  << ", writing the first " << std::dec << total << " bytes anyway" << std::endl;
	}

	data.resize(total);
	Upload(data);

	std::cerr << "Wrote " << std::dec << total << " bytes to the radio" << std::endl;
}

auto UV17ProRadio::ReadCodeplug(const std::string &file) -> void
{
	auto data = Download();

	//CHIRP identifies a raw image by the model name stamped after the
	//memory dump, add it so the file opens in CHIRP as well
	auto model_name = ToBytes(model.chirp_model);
	data.insert(data.end(), model_name.begin(), model_name.end());

	std::ofstream out(file, std::ios_base::out | std::ios_base::binary);
	if (!out.is_open())
	{
		throw std::runtime_error("Cant open file for writing: " + file);
	}
	out.write((const char *)data.data(), data.size());
	out.close();

	std::cerr << "Wrote " << std::dec << data.size() << " bytes to " << file << std::endl;
}

auto UV17ProRadio::ToString() const -> const std::string
{
	Identify();

	std::string info;
	for (const auto &c : ident_response)
	{
		info += (c >= 0x20 && c < 0x7f) ? (char)c : '.';
	}

	std::stringstream out;
	out << "Model:  Baofeng " << model.name << std::endl
		<< "Ident:  " << info;
	return out.str();
}
