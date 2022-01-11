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

namespace radio_tool::radio
{
	class RadioInfo
	{
	public:
		const std::wstring manufacturer, product;
		const uint16_t vid, pid, index;
		const RadioDeviceDriver driver;

		RadioInfo(
			const std::wstring& mfg, 
			const std::wstring& prd, 
			const uint16_t& vid, 
			const uint16_t& pid, 
			const uint16_t& idx, 
			const RadioDeviceDriver& drv)
			: manufacturer(mfg), product(prd), vid(vid), pid(pid), index(idx), driver(drv) {}

		auto ToString() const -> const std::wstring
		{
			std::wstringstream os;
			os << L"["
				<< std::setfill(L'0') << std::setw(4) << std::hex << vid
				<< L":"
				<< std::setfill(L'0') << std::setw(4) << std::hex << pid
				<< L"]: idx=" << std::setfill(L'0') << std::setw(3) << std::to_wstring(index) << L", "
				<< manufacturer << L" " << product << L", "
				<< L"driver=" << DriverNames.at((int)driver);
			return os.str();
		}
	};

	const auto DriverNames = std::vector<std::wstring>{
		L"TYT",
		L"YModem"
	};

	enum class RadioDeviceDriver {
		TYT, 
		YModem
	};

	class RadioOperations
	{
	public:
		virtual ~RadioOperations() = default;

		/**
		 * Write a firmware file to the device (Firmware Upgrade)
		 */
		virtual auto WriteFirmware(const std::string& file) const -> void = 0;

		//virtual auto WriteCodeplug();
		//virtual auto ReadCodeplug();

		/**
		 * Return the device communication handler
		 */
		virtual auto GetDevice() const -> const device::RadioDevice & = 0;

		/**
		 * Get general info about the radio
		 */
		virtual auto ToString() const -> const std::string = 0;
	};
} // namespace radio_tool::radio