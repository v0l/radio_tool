/**
 * This file is part of radio_tool.
 * Copyright (c) 2026 v0l <radio_tool@v0l.io>
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

#include <radio_tool/device/byte_stream.hpp>

#include <condition_variable>
#include <deque>
#include <memory>
#include <mutex>
#include <string>
#include <vector>

namespace SimpleBLE
{
	class Peripheral;
} // namespace SimpleBLE

namespace radio_tool::device
{
	/**
	 * One BLE radio seen while scanning
	 */
	struct BleDevice
	{
		std::string address;
		std::string name;
	};

	/**
	 * A byte stream over a BLE serial service, the arrangement used by radios
	 * which have no programming cable at all (Baofeng UV-17Pro family).
	 *
	 * The radio exposes one characteristic it accepts writes on and one it
	 * sends notifications on, which together behave like a serial port. The
	 * notified bytes are queued as they arrive, so Read can hand out whatever
	 * the radio has sent so far without caring how it was split across
	 * notifications.
	 */
	class BlePort : public ByteStream
	{
	public:
		/**
		 * Connect to a radio by address, discovering the serial service
		 * unless explicit characteristic UUIDs are given
		 */
		BlePort(const std::string &address, const std::string &adapter = "",
				const uint32_t &scan_ms = 15000,
				const std::string &write_uuid = "", const std::string &notify_uuid = "");
		~BlePort();

		BlePort(const BlePort &) = delete;
		auto operator=(const BlePort &) -> BlePort & = delete;

		auto Write(const std::vector<uint8_t> &data) const -> void override;
		auto Read(const size_t &len, const uint32_t &timeout_ms) const -> std::vector<uint8_t> override;
		auto FlushInput() const -> void override;

		/**
		 * Radios in this family expect larger blocks over BLE than over a
		 * cable, matching what CHIRP sends
		 */
		auto BlockSizeHint() const -> size_t override
		{
			return 0x80;
		}

		/**
		 * The service and characteristic we ended up talking to
		 */
		auto ToString() const -> std::string;

		/**
		 * Scan for BLE devices which are advertising a name
		 */
		static auto Scan(const uint32_t &scan_ms = 10000, const std::string &adapter = "") -> std::vector<BleDevice>;

		/**
		 * Names of the Bluetooth adapters on this machine
		 */
		static auto Adapters() -> std::vector<std::string>;

	private:
		auto Receive(const std::vector<uint8_t> &data) const -> void;

		std::unique_ptr<SimpleBLE::Peripheral> peripheral;
		std::string service, write_char, notify_char;
		bool write_with_response;

		//filled by the notification callback, drained by Read
		mutable std::mutex rx_lock;
		mutable std::condition_variable rx_signal;
		mutable std::deque<uint8_t> rx;
	};
} // namespace radio_tool::device
