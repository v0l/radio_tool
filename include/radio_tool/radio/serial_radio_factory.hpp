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

#include <radio_tool/radio/radio_factory.hpp>
#include <radio_tool/radio/radio.hpp>

#include <string>
#include <vector>
#include <memory>

#include <libusb-1.0/libusb.h>

namespace radio_tool::radio {
	class SerialRadioFactory : public RadioOperationsFactory
	{
	public:
		/**
		 * Return the radio support handler for a specified usb device
		 */
		auto GetRadioSupport(const uint16_t& idx) const->std::unique_ptr<RadioOperations> override;

		/**
		 * Gets info about currently supported devices
		 */
		auto ListDevices() const -> const std::vector<RadioInfo> override;

	private:
		auto GetDeviceString(const uint8_t&, libusb_device_handle*) const->std::wstring;
		auto OpDeviceList(std::function<void(const libusb_device*, const libusb_device_descriptor&, const uint16_t&)>) const -> void;
	};
}