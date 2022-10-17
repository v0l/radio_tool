/**
 * This file is part of radio_tool.
 * Copyright (c) 2022 v0l <radio_tool@v0l.io>
 *                    Niccolò Izzo <iu2kin@openrtx.org>
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
#include <radio_tool/radio/yaesu_radio.hpp>
#include <radio_tool/fw/yaesu_fw.hpp>
#include <radio_tool/util/flash.hpp>

#include <math.h>
#include <iomanip>
#include <iostream>
#include <vector>

using namespace radio_tool::radio;

auto YaesuRadio::ToString() const -> const std::string
{
	std::stringstream out;

	auto model = h8sx.IdentifyDevice();

	out << "== Yaesu Radio Info ==" << std::endl
		<< "Radio: " << model << std::endl;

	return out.str();
}

auto YaesuRadio::WriteFirmware(const std::string& file) -> void
{
	auto fw = fw::YaesuFW();
	fw.Read(file);

	auto to_write = fw.GetData();
	h8sx.Init();
	h8sx.IdentifyDevice();
	h8sx.Download(to_write);
}
