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
#include <radio_tool/device/tyt_sgl_device.hpp>

using namespace radio_tool::device;

auto TYTSGLDevice::SetAddress(const uint32_t &) const -> void
{

}

auto TYTSGLDevice::Erase(const uint32_t &amount) const -> void
{

}

auto TYTSGLDevice::Write(const std::vector<uint8_t> &data) const -> void
{

}

auto TYTSGLDevice::Read(const uint16_t &size) const -> std::vector<uint8_t>
{
    return { 0 };
}

auto TYTSGLDevice::Status() const -> const std::string
{
    return "Unknown";
}