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
#include <thread>

#include <libusb-1.0/libusb.h>

namespace radio_tool::radio
{
	class USBRadioInfo : public RadioInfo
	{
	public:
		const uint16_t vid, pid;

		USBRadioInfo(
			const CreateRadioOps l,
			const std::wstring &mfg,
			const std::wstring &prd,
			const uint16_t &vid,
			const uint16_t &pid,
			const uint16_t &idx)
			: RadioInfo(idx, mfg, prd, ""), vid(vid), pid(pid), loader(l)
			{}

		auto ToString() const -> const std::wstring override
		{
			std::wstringstream os;
			os << L"["
			   << std::setfill(L'0') << std::setw(4) << std::hex << vid
			   << L":"
			   << std::setfill(L'0') << std::setw(4) << std::hex << pid
			   << L"]: idx=" << std::setfill(L'0') << std::setw(3) << std::to_wstring(index) << L", "
			   << manufacturer << L" " << model;
			return os.str();
		}

		auto OpenDevice() const -> RadioOperations* override
		{
			return loader();
		}

	private:
		const CreateRadioOps loader;
	};

	/**
	 * libusb devices are enumerated from here,
	 * implementors of direct usb drivers with libusb can hook this factory
	 */
	class USBRadioFactory : public RadioOperationsFactory
	{
	public:
		USBRadioFactory();
		~USBRadioFactory();
		auto ListDevices(const uint16_t& idx_offset) const -> const std::vector<RadioInfo*> override;
		auto HandleEvents() -> void;
	private:
		auto GetDeviceString(const uint8_t &, libusb_device_handle *) const -> std::wstring;
		static auto OpenDevice(const uint8_t &bus, const uint8_t &port, const uint8_t& address) -> libusb_device_handle *;
		static auto CreateContext() -> libusb_context *;

		libusb_context* usb_ctx;
		std::thread events;
	};
}
