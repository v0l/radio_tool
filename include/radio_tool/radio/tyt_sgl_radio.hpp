/**
 * This file is part of radio_tool.
 * Copyright (c) 2021 v0l <radio_tool@v0l.io>
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
#include <radio_tool/hid/tyt_hid.hpp>

#include <functional>

namespace radio_tool::radio
{
	class TYTSGLRadio : public RadioOperations
	{
	public:
		TYTSGLRadio(libusb_device_handle* h);

		auto WriteFirmware(const std::string& file) -> void override;
		auto ToString() const -> const std::string override;

		static auto SupportsDevice(const libusb_device_descriptor& dev) -> bool
		{
			if (dev.idVendor == hid::TYTHID::VID && dev.idProduct == hid::TYTHID::PID)
			{
				return true;
			}
			return false;
		}

		static auto Create(libusb_device_handle* h) -> TYTSGLRadio*
		{
			return new TYTSGLRadio(h);
		}
	private:
		hid::TYTHID device;
		auto checksum(std::vector<uint8_t>::const_iterator&& begin, std::vector<uint8_t>::const_iterator&& end) const -> uint32_t;
	};
} // namespace radio_tool::radio