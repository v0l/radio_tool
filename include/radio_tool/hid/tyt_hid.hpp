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

#include <libusb-1.0/libusb.h>
#include <radio_tool/hid/hid.hpp>

#include <mutex>
#include <condition_variable>

namespace radio_tool::hid
{
	namespace tyt
	{
		namespace commands
		{
			const std::vector<uint8_t> A = { 'A' };
			const std::vector<uint8_t> Update = { '#', 'U', 'P', 'D', 'A', 'T', 'E', '?' };
			const std::vector<uint8_t> Download = { 'D', 'O', 'W', 'N', 'L', 'O', 'A', 'D' };
			const std::vector<uint8_t> FlashProgram = { 'F', '-', 'P', 'R', 'O', 'G' };
			const std::vector<uint8_t> FlashErase = { 'F', '-', 'E', 'R', 'A', 'S', 'E' };
			const std::vector<uint8_t> EraseROM = { 'E', 'R', 'A', 'S', 'E', 'R', 'O', 'M' };
			const std::vector<uint8_t> FlashVersion = { 'F', '-', 'V', 'E', 'R' };
			const std::vector<uint8_t> FlashCompany = { 'F', '-', 'C', 'O' };
			const std::vector<uint8_t> FlashSerialNumber = { 'F', '-', 'S', 'N' };
			const std::vector<uint8_t> FlashTime = { 'F', '-', 'T', 'I', 'M', 'E' }; // ? write time ?
			const std::vector<uint8_t> FlashMod = { 'F', '-', 'M', 'O', 'D' };       // ? mode ?
			const std::vector<uint8_t> Program = { 'P', 'R', 'O', 'G', 'R', 'A', 'M' };
			const std::vector<uint8_t> End = { 'E', 'N', 'D' };
		};

		enum class CommandType : uint16_t
		{
			HostToDevice = 0x01,
			DeviceToHost = 0x03
		};

		class Command
		{
		public:
			Command(const CommandType& t, const uint16_t& l, const std::vector<uint8_t>& d)
				: type(t), length(l), data(d) {}

			const CommandType type;
			const uint16_t length;
			const std::vector<uint8_t> data;

			auto operator==(const Command& other) const -> bool
			{
				return type == other.type && length == other.length && std::equal(data.begin(), data.end(), other.data.begin());
			}
		};

		/**
		 * OK response to device
		 */
		const Command OK = Command(CommandType::HostToDevice, 1, commands::A);

		/**
		 * OK response from device
		 */
		const Command OKResponse = Command(CommandType::DeviceToHost, 1, commands::A);
	};

	class TYTHID : public HID
	{
	public:
		const unsigned char EP_IN = LIBUSB_ENDPOINT_IN | 0x01;
		const unsigned char EP_OUT = LIBUSB_ENDPOINT_OUT | 0x02;

		static const auto VID = 0x15a2;
		static const auto PID = 0x0073;

		TYTHID(libusb_device_handle* device)
			: HID(device), signalCallback(), signalReady(), tx(nullptr) {}

		auto Setup() -> void;

		auto SendCommand(const tyt::Command& cmd) -> void;
		auto SendCommand(const std::vector<uint8_t>& cmd) -> void;
		auto SendCommand(const std::vector<uint8_t>& cmd, const uint8_t& size, const uint8_t& fill) -> void;

		auto SendCommandAndOk(const tyt::Command& cmd) -> void;
		auto SendCommandAndOk(const std::vector<uint8_t>& cmd) -> void;
		auto SendCommandAndOk(const std::vector<uint8_t>& cmd, const uint8_t& size, const uint8_t& fill) -> void;

		auto WaitForReply()->tyt::Command;

		auto OnTransfer(libusb_transfer* tx) -> void;

	private:
		std::mutex signalCallback;
		std::condition_variable signalReady;
		struct libusb_transfer* tx;
	};
}