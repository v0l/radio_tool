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

#include <vector>

namespace radio_tool::device
{
	class RadioDevice
	{
	public:
		/**
		 * Set the device read/write address
		 */
		virtual auto SetAddress(const uint32_t&) const -> void = 0;

		/**
		 * Erase bytes from the specified address
		 */
		virtual auto Erase(const uint32_t& addr) const -> void = 0;

		/**
		 * Write bytes to the device
		 */
		virtual auto Write(const std::vector<uint8_t>& data) const -> void = 0;

		/**
		 * Read bytes from the device
		 */
		virtual auto Read(const uint16_t& size) const->std::vector<uint8_t> = 0;
	};

} // namespace radio_tool::device