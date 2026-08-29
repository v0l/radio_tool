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
#include <radio_tool/radio/uv5r_radio.hpp>

#include <fstream>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <stdexcept>

using namespace radio_tool::radio;
using namespace radio_tool::device;

namespace
{
	constexpr auto ACK = 0x06;
	constexpr auto MainMemoryEnd = 0x1800;
	constexpr auto AuxMemoryStart = 0x1EC0;
	constexpr auto AuxMemoryEnd = 0x2000;
	constexpr auto BlockSize = 0x40;
	constexpr auto ReadTimeout = 1000u;

	auto Trim(const std::vector<uint8_t> &data) -> std::string
	{
		std::string ret;
		for (const auto &c : data)
		{
			if (c == 0x00 || c == 0xff)
			{
				break;
			}
			ret += (c >= 0x20 && c < 0x7f) ? (char)c : '.';
		}
		return ret;
	}

	auto Hex(const std::vector<uint8_t> &data) -> std::string
	{
		std::stringstream ss;
		for (const auto &c : data)
		{
			ss << std::hex << std::setw(2) << std::setfill('0') << (int)c;
		}
		return ss.str();
	}
}

auto UV5RRadio::Models() -> const std::vector<UV5RModel> &
{
	//magic values from chirp/drivers/uv5r.py
	static const std::vector<UV5RModel> models = {
		{"UV5R",
		 {{0x50, 0xBB, 0xFF, 0x20, 0x12, 0x07, 0x25},  //UV5R_MODEL_291
		  {0x50, 0xBB, 0xFF, 0x01, 0x25, 0x98, 0x4D}}, //UV5R_MODEL_ORIG
		 true},
		{"UV82",
		 {{0x50, 0xBB, 0xFF, 0x20, 0x13, 0x01, 0x05}},
		 true},
		{"UV6",
		 {{0x50, 0xBB, 0xFF, 0x20, 0x12, 0x08, 0x23},
		  {0x50, 0xBB, 0xFF, 0x12, 0x03, 0x98, 0x4D}},
		 true},
		{"F11",
		 {{0x50, 0xBB, 0xFF, 0x13, 0xA1, 0x11, 0xDD}},
		 false},
	};
	return models;
}

auto UV5RRadio::GetModel(const std::string &name) -> const UV5RModel *
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

UV5RRadio::UV5RRadio(const std::string &prt, const UV5RModel &model)
	: port(prt, BaudRate), model(model)
{
}

auto UV5RRadio::WriteFirmware(const std::string &) -> void
{
	throw std::runtime_error("Firmware upgrade is not supported for this radio");
}

auto UV5RRadio::TryIdent(const std::vector<uint8_t> &magic) const -> std::vector<uint8_t>
{
	port.FlushInput();

	//the radio drops the magic if it arrives too fast
	port.WriteSlow(magic, 10);

	auto ack = port.Read(1, ReadTimeout);
	if (ack.size() != 1 || ack[0] != ACK)
	{
		throw std::runtime_error("No response from radio, check the cable is seated fully and the radio is switched on");
	}

	port.Write({0x02});

	//the ident is normally 8 bytes but some radios send 12, read until
	//the terminator so both layouts end up 8 bytes long
	std::vector<uint8_t> response;
	for (auto ix = 0; ix < 12; ix++)
	{
		auto b = port.Read(1, ReadTimeout);
		if (b.empty())
		{
			break;
		}
		response.push_back(b[0]);
		if (b[0] == 0xDD)
		{
			break;
		}
	}

	std::vector<uint8_t> id;
	if (response.size() == 8)
	{
		id = response;
	}
	else if (response.size() == 12)
	{
		id = {response[0], response[3], response[5]};
		id.insert(id.end(), response.begin() + 7, response.end());
	}
	else
	{
		throw std::runtime_error("Unexpected ident from radio: " + Hex(response));
	}

	port.Write({ACK});
	ack = port.Read(1, ReadTimeout);
	if (ack.size() != 1 || ack[0] != ACK)
	{
		throw std::runtime_error("Radio refused clone");
	}

	return id;
}

auto UV5RRadio::Identify() const -> void
{
	if (!ident.empty())
	{
		return;
	}

	std::string last_error;
	for (const auto &magic : model.idents)
	{
		try
		{
			ident = TryIdent(magic);
			return;
		}
		catch (const std::runtime_error &e)
		{
			last_error = e.what();
			//the radio needs a moment before it will listen again
			SerialPort::Sleep(2000);
		}
	}

	throw std::runtime_error("Failed to identify radio: " + last_error);
}

auto UV5RRadio::ReadBlock(const uint16_t &addr, const uint8_t &size, const bool &first) const -> std::vector<uint8_t>
{
	std::vector<uint8_t> cmd = {(uint8_t)'S', (uint8_t)(addr >> 8), (uint8_t)(addr & 0xff), size};
	port.Write(cmd);

	if (!first)
	{
		auto ack = port.Read(1, ReadTimeout);
		if (ack.size() != 1 || ack[0] != ACK)
		{
			std::stringstream err;
			err << "Radio refused to send block 0x" << std::hex << std::setw(4) << std::setfill('0') << addr;
			throw std::runtime_error(err.str());
		}
	}

	std::stringstream what;
	what << "header for block 0x" << std::hex << std::setw(4) << std::setfill('0') << addr;
	auto hdr = port.ReadExact(4, ReadTimeout, what.str());

	auto r_addr = (uint16_t)((hdr[1] << 8) | hdr[2]);
	if (hdr[0] != (uint8_t)'X' || r_addr != addr || hdr[3] != size)
	{
		std::stringstream err;
		err << "Unexpected response for block 0x" << std::hex << std::setw(4) << std::setfill('0') << addr
			<< " (cmd=" << (int)hdr[0] << " addr=0x" << r_addr << " size=0x" << (int)hdr[3] << ")";
		throw std::runtime_error(err.str());
	}

	std::stringstream what_data;
	what_data << "block 0x" << std::hex << std::setw(4) << std::setfill('0') << addr;
	auto data = port.ReadExact(size, ReadTimeout, what_data.str());

	port.Write({ACK});
	SerialPort::Sleep(50);

	return data;
}

auto UV5RRadio::ReadFirmwareVersion() const -> void
{
	//this walks the clone protocol, so it must only run once per session,
	//otherwise --info followed by a download would leave the radio out of step
	if (firmware_read)
	{
		return;
	}
	firmware_read = true;

	//new radios reply with junk if the aux area is read first, so read a
	//block outside it before touching 0x1EC0
	ReadBlock(0x1E80, BlockSize, true);
	auto block1 = ReadBlock(0x1EC0, BlockSize, false);
	auto block2 = ReadBlock(0x1FC0, BlockSize, false);

	firmware_version = Trim(std::vector<uint8_t>(block1.begin() + 48, block1.begin() + 62));

	//some radios drop the byte at 0x1FCF when the last block is read in one
	//0x40 byte go, they need the tail reading in 0x10 byte blocks instead
	dropped_byte = block2[15] == 0xff;
}

auto UV5RRadio::Download() const -> std::vector<uint8_t>
{
	if (data_read)
	{
		throw std::runtime_error("The codeplug has already been downloaded in this session");
	}
	data_read = true;

	Identify();

	if (model.aux_block)
	{
		ReadFirmwareVersion();
	}
	else
	{
		ReadBlock(0x0000, BlockSize, true);
	}

	auto data = ident;

	std::cerr << "Downloading main memory..." << std::endl;
	for (auto i = 0; i < MainMemoryEnd; i += BlockSize)
	{
		auto block = ReadBlock((uint16_t)i, BlockSize, false);
		data.insert(data.end(), block.begin(), block.end());

		std::cerr << "\r 0x" << std::hex << std::setw(4) << std::setfill('0') << i
				  << " / 0x" << MainMemoryEnd << std::dec << std::flush;
	}
	std::cerr << std::endl;

	if (model.aux_block)
	{
		std::cerr << "Downloading aux memory..." << std::endl;
		if (dropped_byte)
		{
			for (auto i = AuxMemoryStart; i < 0x1FC0; i += BlockSize)
			{
				auto block = ReadBlock((uint16_t)i, BlockSize, false);
				data.insert(data.end(), block.begin(), block.end());
			}
			for (auto i = 0x1FC0; i < AuxMemoryEnd; i += 0x10)
			{
				auto block = ReadBlock((uint16_t)i, 0x10, false);
				data.insert(data.end(), block.begin(), block.end());
			}
		}
		else
		{
			for (auto i = AuxMemoryStart; i < AuxMemoryEnd; i += BlockSize)
			{
				auto block = ReadBlock((uint16_t)i, BlockSize, false);
				data.insert(data.end(), block.begin(), block.end());
			}
		}
	}

	return data;
}

auto UV5RRadio::ReadCodeplug(const std::string &file) -> void
{
	auto data = Download();

	std::ofstream out(file, std::ios_base::out | std::ios_base::binary);
	if (!out.is_open())
	{
		throw std::runtime_error("Cant open file for writing: " + file);
	}
	out.write((const char *)data.data(), data.size());
	out.close();

	std::cerr << "Wrote " << std::dec << data.size() << " bytes to " << file << std::endl;
}

auto UV5RRadio::ToString() const -> const std::string
{
	Identify();
	if (model.aux_block)
	{
		ReadFirmwareVersion();
	}

	std::stringstream out;
	out << "Model:    Baofeng " << model.name << std::endl
		<< "Ident:    " << Hex(ident) << std::endl
		<< "Firmware: " << (firmware_version.empty() ? "Unknown" : firmware_version);
	return out.str();
}
