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

#include <radio_tool/codeplug/codeplug.hpp>

#include <memory>
#include <string>
#include <vector>

#include <stdint.h>

namespace radio_tool::codeplug
{
	/**
	 * A single memory channel as stored in a UV-5R image
	 */
	struct UV5RChannel
	{
		bool empty = true;
		uint32_t rx_freq = 0; //Hz
		uint32_t tx_freq = 0; //Hz
		bool tx_inhibit = false;
		std::string name;
		uint16_t rx_tone_raw = 0;
		uint16_t tx_tone_raw = 0;
		bool wide = true;
		bool scan = false;
		bool bcl = false;
		uint8_t power = 0; //0 = high, 1 = low
		uint8_t pttid = 0;
		uint8_t scode = 0;
	};

	/**
	 * Decode a raw tone value into "88.5", "D023N" or "none"
	 */
	auto UV5RToneToString(const uint16_t &raw) -> const std::string;

	/**
	 * Baofeng UV-5R family codeplug image, the same layout CHIRP uses:
	 * 8 byte ident followed by the radio memory, so every documented radio
	 * address is offset by 8 in the file
	 *
	 * https://github.com/kk7ds/chirp/blob/master/chirp/drivers/uv5r.py
	 */
	class UV5R : public CodeplugSupport
	{
	public:
		static constexpr auto IdentSize = 0x08u;
		static constexpr auto ChannelCount = 128u;
		static constexpr auto ChannelsOffset = 0x0008u;
		static constexpr auto NamesOffset = 0x1008u;
		static constexpr auto SettingsOffset = 0x0E28u;
		static constexpr auto PowerOnMsgOffset = 0x1828u;
		static constexpr auto FirmwareOffset = 0x1838u;

		static auto SupportsCodeplug(const std::string &file) -> bool;

		static auto Create() -> std::unique_ptr<UV5R>
		{
			return std::unique_ptr<UV5R>(new UV5R());
		}

		auto Read(const std::string &) -> void override;
		auto Write(const std::string &) const -> void override;
		auto GetData() const -> const std::vector<uint8_t> override;
		auto ToString() const -> const std::string override;

		auto GetChannels() const -> const std::vector<UV5RChannel> &
		{
			return channels;
		}

	private:
		auto Parse() -> void;

		std::vector<uint8_t> data;
		std::vector<UV5RChannel> channels;
		std::string firmware_version, power_on_msg;
	};
} // namespace radio_tool::codeplug
