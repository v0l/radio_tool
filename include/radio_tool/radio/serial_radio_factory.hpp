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
#include <functional>

namespace radio_tool::radio
{
	class SerialRadioInfo : public RadioInfo
	{
	public:
		SerialRadioInfo(const std::wstring &p, const uint16_t &idx) : index(idx), port(p) {}

		auto ToString() const -> const std::wstring override
		{
			std::wstringstream os;
			os << L"[" << port << "]: idx=" << std::to_wstring(index) << L", "
			   << L"Generic serial radio";

			return os.str();
		}

	private:
		const uint16_t index;
		const std::wstring port;
	};

	class SerialRadioFactory : public RadioOperationsFactory
	{
	public:
		/**
		 * Return the radio support handler for a specified usb device
		 */
		auto GetRadioSupport(const uint16_t &idx) const -> const RadioOperations * override;

		/**
		 * Gets info about currently supported devices
		 */
		auto ListDevices(const uint16_t &idx_offset) const -> const std::vector<RadioInfo *> override;

	private:
		auto OpDeviceList(std::function<void(const std::wstring &, const uint16_t &)>) const -> void;
	};
}