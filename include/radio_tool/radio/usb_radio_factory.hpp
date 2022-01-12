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

#include <string>
#include <vector>
#include <memory>
#include <functional>

#include <libusb-1.0/libusb.h>

namespace radio_tool::radio {
	class USBRadioInfo : public RadioInfo {
	public:
		const std::wstring manufacturer, product;
		const uint16_t vid, pid, index;

		USBRadioInfo(
			const std::wstring& mfg,
			const std::wstring& prd,
			const uint16_t& vid,
			const uint16_t& pid,
			const uint16_t& idx)
			: manufacturer(mfg), product(prd), vid(vid), pid(pid), index(idx) {}

		auto ToString() const -> const std::wstring override
		{
			std::wstringstream os;
			os << L"["
				<< std::setfill(L'0') << std::setw(4) << std::hex << vid
				<< L":"
				<< std::setfill(L'0') << std::setw(4) << std::hex << pid
				<< L"]: idx=" << std::setfill(L'0') << std::setw(3) << std::to_wstring(index) << L", "
				<< manufacturer << L" " << product;
			return os.str();
		}
	};

	class USBRadioFactory : public RadioOperationsFactory
	{
	public:
		USBRadioFactory();
		~USBRadioFactory();

		/**
		 * Return the radio support handler for a specified usb device
		 */
		auto GetRadioSupport(const uint16_t& idx) const -> const RadioOperations* override;

		/**
		 * Gets info about currently supported devices
		 */
		auto ListDevices(const uint16_t& idx_offset) const -> const std::vector<RadioInfo*> override;

	private:
		auto GetDeviceString(const uint8_t&, libusb_device_handle*) const->std::wstring;
		auto OpDeviceList(std::function<void(const libusb_device*, const libusb_device_descriptor&, const uint16_t&)>) const -> void;

		libusb_context* usb_ctx;
	};
}