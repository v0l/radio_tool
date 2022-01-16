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

#include <string>
#include <sstream>
#include <iomanip>
#include <functional>

namespace radio_tool::radio
{
	class RadioOperations;
	typedef std::function<const RadioOperations *()> CreateRadioOps;

	/**
	 * Information related to a detected radio
	 */
	class RadioInfo
	{
	public:
		virtual auto ToString() const -> const std::wstring = 0;
		virtual auto OpenDevice() const -> const RadioOperations* = 0;
	};

	/**
	 * Generic interface for operations which we want to perform on radios
	 */
	class RadioOperations
	{
	public:
		virtual ~RadioOperations() = default;

		/**
		 * Write a firmware file to the device (Firmware Upgrade)
		 */
		virtual auto WriteFirmware(const std::string &file) const -> void = 0;

		//virtual auto WriteCodeplug();
		//virtual auto ReadCodeplug();

		/**
		 * Return the device communication handler
		 */
		virtual auto GetDevice() const -> const device::RadioDevice * = 0;

		/**
		 * Get general info about the radio
		 */
		virtual auto ToString() const -> const std::string = 0;
	};

	/**
	 * Interface for listing devices
	 */
	class RadioOperationsFactory
	{
	public:
		virtual auto ListDevices(const uint16_t &idx_offset) const -> const std::vector<RadioInfo *> = 0;
	};
} // namespace radio_tool::radio