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
#include <radio_tool/device/yaesu_device.hpp>

using namespace radio_tool::device;

auto YaesuDevice::SetAddress(const uint32_t &) const -> void { }
auto YaesuDevice::Erase(const uint32_t &) const -> void { }
auto YaesuDevice::Read(const uint16_t &) const -> std::vector<uint8_t>
{
    return { 0 };
}
auto YaesuDevice::Status() const -> const std::string
{
    return "";
}

auto YaesuDevice::Write(const std::vector<uint8_t>& data) const -> void
{
	this->h8sx.Download(data);
}
