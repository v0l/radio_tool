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
#include <radio_tool/codeplug/uv5r.hpp>

#include <fstream>
#include <iomanip>
#include <sstream>
#include <stdexcept>

using namespace radio_tool::codeplug;

namespace
{
	//chirp_common.DTCS_CODES plus 645, which the UV-5R supports
	const std::vector<uint16_t> DTCSCodes = {
		23, 25, 26, 31, 32, 36, 43, 47, 51, 53, 54,
		65, 71, 72, 73, 74, 114, 115, 116, 122, 125, 131,
		132, 134, 143, 145, 152, 155, 156, 162, 165, 172, 174,
		205, 212, 223, 225, 226, 243, 244, 245, 246, 251, 252,
		255, 261, 263, 265, 266, 271, 274, 306, 311, 315, 325,
		331, 332, 343, 346, 351, 356, 364, 365, 371, 411, 412,
		413, 423, 431, 432, 445, 446, 452, 454, 455, 462, 464,
		465, 466, 503, 506, 516, 523, 526, 532, 546, 565, 606,
		612, 624, 627, 631, 632, 645, 654, 662, 664, 703, 712,
		723, 731, 732, 734, 743, 754};

	auto ReadU16LE(const std::vector<uint8_t> &d, const size_t &offset) -> uint16_t
	{
		return (uint16_t)(d[offset] | (d[offset + 1] << 8));
	}

	/**
	 * Little endian BCD frequency, stored in units of 10Hz
	 */
	auto ReadBCDFreq(const std::vector<uint8_t> &d, const size_t &offset) -> uint32_t
	{
		uint32_t val = 0;
		for (auto ix = 4; ix > 0; ix--)
		{
			auto b = d[offset + ix - 1];
			val = (val * 100) + (((b >> 4) & 0x0f) * 10) + (b & 0x0f);
		}
		return val * 10;
	}

	auto ReadName(const std::vector<uint8_t> &d, const size_t &offset, const size_t &len) -> std::string
	{
		std::string ret;
		for (size_t ix = 0; ix < len; ix++)
		{
			auto c = d[offset + ix];
			if (c == 0x00)
			{
				break;
			}
			//the vendor software pads with 0xff, sometimes mid name
			ret += (c == 0xff) ? ' ' : (char)c;
		}
		while (!ret.empty() && ret.back() == ' ')
		{
			ret.pop_back();
		}
		return ret;
	}

	auto FormatFreq(const uint32_t &hz) -> std::string
	{
		std::stringstream ss;
		ss << std::fixed << std::setprecision(5) << (hz / 1000000.0);
		return ss.str();
	}
}

auto radio_tool::codeplug::UV5RToneToString(const uint16_t &raw) -> const std::string
{
	if (raw == 0 || raw == 0xffff)
	{
		return "none";
	}

	if (raw >= 0x0258)
	{
		//CTCSS, stored in tenths of a Hz
		std::stringstream ss;
		ss << std::fixed << std::setprecision(1) << (raw / 10.0);
		return ss.str();
	}

	//DTCS, values over 0x69 are the inverted set
	size_t index = 0;
	char polarity = 'N';
	if (raw > 0x69)
	{
		index = raw - 0x6A;
		polarity = 'R';
	}
	else
	{
		index = raw - 1;
	}

	if (index >= DTCSCodes.size())
	{
		std::stringstream ss;
		ss << "?0x" << std::hex << raw;
		return ss.str();
	}

	std::stringstream ss;
	ss << "D" << std::setw(3) << std::setfill('0') << DTCSCodes[index] << polarity;
	return ss.str();
}

auto UV5R::SupportsCodeplug(const std::string &file) -> bool
{
	std::ifstream f(file, std::ios_base::in | std::ios_base::binary);
	if (!f.is_open())
	{
		return false;
	}

	f.seekg(0, f.end);
	auto size = (size_t)f.tellg();
	f.seekg(0, f.beg);

	//sizes CHIRP accepts for this family
	if (size != 0x1808 && size != 0x1948 && size != 0x1950)
	{
		return false;
	}

	uint8_t id[3] = {};
	f.read((char *)id, sizeof(id));
	if (!f.good())
	{
		return false;
	}

	//every ident in this family starts with the same 3 bytes
	return id[0] == 0x50 && id[1] == 0xBB && id[2] == 0xFF;
}

auto UV5R::Read(const std::string &file) -> void
{
	std::ifstream f(file, std::ios_base::in | std::ios_base::binary);
	if (!f.is_open())
	{
		throw std::runtime_error("Cant open file");
	}

	f.seekg(0, f.end);
	auto size = (size_t)f.tellg();
	f.seekg(0, f.beg);

	data.resize(size);
	f.read((char *)data.data(), size);
	if (!f.good())
	{
		throw std::runtime_error("Failed to read codeplug");
	}
	f.close();

	Parse();
}

auto UV5R::Parse() -> void
{
	if (data.size() < NamesOffset + (ChannelCount * 0x10))
	{
		throw std::runtime_error("Codeplug is too small to be a UV-5R image");
	}

	channels.clear();
	channels.reserve(ChannelCount);

	for (auto ix = 0u; ix < ChannelCount; ix++)
	{
		auto mem = ChannelsOffset + (ix * 0x10);
		auto nam = NamesOffset + (ix * 0x10);

		UV5RChannel ch;
		if (data[mem] == 0xff)
		{
			channels.push_back(ch);
			continue;
		}

		ch.empty = false;
		ch.rx_freq = ReadBCDFreq(data, mem);
		ch.tx_inhibit = data[mem + 4] == 0xff && data[mem + 5] == 0xff &&
						data[mem + 6] == 0xff && data[mem + 7] == 0xff;
		ch.tx_freq = ch.tx_inhibit ? 0 : ReadBCDFreq(data, mem + 4);
		ch.rx_tone_raw = ReadU16LE(data, mem + 8);
		ch.tx_tone_raw = ReadU16LE(data, mem + 10);
		ch.scode = data[mem + 12] & 0x0f;
		ch.power = (data[mem + 14] & 0x03);

		auto flags = data[mem + 15];
		ch.wide = (flags & 0x40) != 0;
		ch.bcl = (flags & 0x08) != 0;
		ch.scan = (flags & 0x04) != 0;
		ch.pttid = flags & 0x03;

		ch.name = ReadName(data, nam, 7);
		channels.push_back(ch);
	}

	if (data.size() >= FirmwareOffset + 14)
	{
		firmware_version = ReadName(data, FirmwareOffset, 14);
	}
	if (data.size() >= PowerOnMsgOffset + 14)
	{
		auto l1 = ReadName(data, PowerOnMsgOffset, 7);
		auto l2 = ReadName(data, PowerOnMsgOffset + 7, 7);
		power_on_msg = l1;
		if (!l2.empty())
		{
			power_on_msg += " / " + l2;
		}
	}
}

auto UV5R::Write(const std::string &file) const -> void
{
	if (data.empty())
	{
		throw std::runtime_error("No codeplug loaded");
	}

	std::ofstream f(file, std::ios_base::out | std::ios_base::binary);
	if (!f.is_open())
	{
		throw std::runtime_error("Cant open file for writing: " + file);
	}
	f.write((const char *)data.data(), data.size());
	f.close();
}

auto UV5R::GetData() const -> const std::vector<uint8_t>
{
	return data;
}

auto UV5R::ToString() const -> const std::string
{
	std::stringstream out;

	out << " == Baofeng UV-5R Codeplug ==" << std::endl
		<< "Firmware: " << (firmware_version.empty() ? "Unknown" : firmware_version) << std::endl
		<< "Power on: " << (power_on_msg.empty() ? "(none)" : power_on_msg) << std::endl
		<< std::endl
		<< "  # Name       RX Freq    TX Freq    RX Tone  TX Tone  BW    Pwr   Scan" << std::endl;

	auto used = 0u;
	for (auto ix = 0u; ix < channels.size(); ix++)
	{
		const auto &ch = channels[ix];
		if (ch.empty)
		{
			continue;
		}
		used++;

		out << std::setw(3) << std::setfill(' ') << std::dec << ix << " "
			<< std::left << std::setw(10) << ch.name << " "
			<< std::right << std::setw(10) << FormatFreq(ch.rx_freq) << " "
			<< std::setw(10) << (ch.tx_inhibit ? "off" : FormatFreq(ch.tx_freq)) << " "
			<< std::setw(8) << UV5RToneToString(ch.rx_tone_raw) << " "
			<< std::setw(8) << UV5RToneToString(ch.tx_tone_raw) << " "
			<< std::setw(5) << (ch.wide ? "Wide" : "Narrow") << " "
			<< std::setw(5) << (ch.power == 0 ? "High" : "Low") << " "
			<< (ch.scan ? "Yes" : "No") << std::endl;
	}

	out << std::endl
		<< std::dec << used << " of " << channels.size() << " channels used" << std::endl;

	return out.str();
}
