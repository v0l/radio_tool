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
#pragma once

#include <radio_tool/dfu/dfu.hpp>

namespace radio_tool::dfu
{
	/**
	 * Custom radio commands starting with 0x91
	 */
	enum class TYTCommand : uint8_t
	{
		ProgrammingMode = 0x01,
		SetRTC = 0x02,
		Reboot = 0x05,
		FirmwareUpgrade = 0x31
	};

	enum class TYTRegister : uint8_t
	{
		RadioInfo = 0x01, //Radio Model(16 bytes) + 16 bytes of something else
		R_02 = 0x02,      //unknown (4 bytes)
		R_03 = 0x03,      //unknown (24 bytes)
		R_04 = 0x04,      //unknown (8 bytes)
		R_07 = 0x07,      //unknown (16 bytes)
		RTC = 0x08,       //Real time clock (7 bytes)
	};

	class TYTDFU : public DFU
	{
	public:
		static const auto VID = 0x0483;
		static const auto PID = 0xdf11;

		static const auto CustomCommand = 0x91;
		static const auto RegisterCommand = 0xa2;
		static const auto RegisterSize = 1024;

		TYTDFU(libusb_device_handle* h) : DFU(h) {}

		/**
		 * Get the radio model off the device
		 */
		auto IdentifyDevice() const->std::string;

		/**
		 * Read some register off the device
		 */
		auto ReadRegister(const TYTRegister& reg) const->std::vector<uint8_t>;

		/**
		 * Gets the current time from the radio's RTC
		 * This doesn't work during TX/RX
		 */
		auto GetTime() const->time_t;

		/**
		 * Set the time on the radio to the current machine time
		 */
		auto SetTime() const -> void;

		/**
		 * Reboot the device
		 */
		auto Reboot() const -> void;

		/**
		 * Send a TYT Command to the device
		 */
		auto SendTYTCommand(const TYTCommand& cmd) const -> void;
	};
} // namespace radio_tool::dfu