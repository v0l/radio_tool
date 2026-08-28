/**
 * This file is part of radio_tool.
 * Copyright (c) 2020 v0l <radio_tool@v0l.io>
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
#include <radio_tool/codeplug/rdt.hpp>
#include <radio_tool/util.hpp>

#include <sstream>
#include <ctime>

using namespace radio_tool::codeplug;

namespace
{
	/**
	 * Format a timestamp without dereferencing the null ctime() may return
	 * for values it cannot represent
	 */
	auto FormatTimestamp(const time_t& ts) -> std::string
	{
		if (ts == static_cast<time_t>(-1))
		{
			return "Invalid";
		}

		std::tm tm_out = {};
#ifdef _WIN32
		if (localtime_s(&tm_out, &ts) != 0)
		{
			return "Invalid";
		}
#else
		if (localtime_r(&ts, &tm_out) == nullptr)
		{
			return "Invalid";
		}
#endif

		char buf[64] = {};
		if (std::strftime(buf, sizeof(buf), "%a %b %e %H:%M:%S %Y", &tm_out) == 0)
		{
			return "Invalid";
		}
		return std::string(buf);
	}
}

auto RDT::Read(const std::string& file) -> void
{
	std::ifstream file_read(file, std::ios_base::in | std::ios_base::binary);
	if (!file_read.is_open())
	{
		throw std::runtime_error("Cant open file");
	}

	header.Read(file_read);
	if (!header.Validate())
	{
		throw std::runtime_error("Not a valid RDT codeplug");
	}

	//codeplug data starts directly after the header, all offsets are
	//relative to the start of the codeplug data
	const auto data_start = static_cast<std::streamoff>(RDTHeaderSize);

	file_read.seekg(data_start + header.GetTimestampOffset(), std::ios_base::beg);

	uint8_t ts_data[7] = {};
	file_read.read((char*)ts_data, sizeof(ts_data));
	if (!file_read.good())
	{
		throw std::runtime_error("Codeplug is truncated, missing timestamp");
	}
	timestamp = ParseBCDTimestamp(ts_data);

	file_read.seekg(data_start + header.GetGeneralOffset(), std::ios_base::beg);
	if (!file_read.good())
	{
		throw std::runtime_error("Codeplug is truncated, missing general settings");
	}
	general.Read(file_read);
}

auto RDT::Write(const std::string&) const -> void
{
}

auto RDT::GetData() const -> const std::vector<uint8_t>
{
	throw std::runtime_error("Not implemented");
}

auto RDT::ToString() const -> const std::string
{
	std::stringstream out;

	out
		<< " == RDT Codeplug ==" << std::endl
		<< "Radio:   " << header.radio << std::endl
		<< "Created: " << FormatTimestamp(timestamp) << std::endl
		<< "Target:  " << header.target_name << std::endl
		<< general.ToString() << std::endl;
	return out.str();
}