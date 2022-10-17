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

#include <radio_tool/radio/radio.hpp>
#include <radio_tool/h8sx/h8sx.hpp>

#include <functional>
#include <libusb-1.0/libusb.h>

namespace radio_tool::radio
{
	class YaesuRadio : public RadioOperations
	{
	public:
		static const auto VID = 0x045b;
		static const auto PID = 0x0025;

		YaesuRadio(libusb_device_handle* h)
			: h8sx(h) {}

		auto WriteFirmware(const std::string& file) -> void override;
		auto ToString() const -> const std::string override;

		static auto SupportsDevice(const libusb_device_descriptor& dev) -> bool
		{
			return dev.idVendor == VID && dev.idProduct == PID;
		}

		static auto Create(libusb_device_handle* h) -> YaesuRadio* {
			return new YaesuRadio(h);
		}
	private:
		h8sx::H8SX h8sx;
	};
} // namespace radio_tool::radio
