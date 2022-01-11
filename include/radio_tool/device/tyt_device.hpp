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
#pragma once

#include <radio_tool/device/device.hpp>
#include <radio_tool/dfu/tyt_dfu.hpp>

#include <libusb-1.0/libusb.h>

namespace radio_tool::device
{
	class TYTDevice : public RadioDevice 
	{
	public:
		TYTDevice(libusb_device_handle* h) : _dfu(h) { }

		auto SetAddress(const uint32_t&) const -> void override;
		auto Erase(const uint32_t& amount) const -> void override;
		auto Write(const std::vector<uint8_t>& data) const -> void override;
		auto Read(const uint16_t& size) const->std::vector<uint8_t> override;

	private:
		dfu::TYTDFU _dfu;
	};
} // namespace radio_tool::device