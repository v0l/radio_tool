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
#include <radio_tool/util.hpp>

#include <string>
#include <vector>
#include <functional>

namespace radio_tool::radio
{
	class SerialRadioInfo : public RadioInfo
	{
	public:
		SerialRadioInfo(
			const CreateRadioOps l,
			const std::string &p,
			const uint16_t &idx)
			: RadioInfo(idx, L"Unknown", L"Unknown", p), loader(l) {}

		auto ToString() const -> const std::wstring override
		{
			std::wstringstream os;
			os << L"[" << std::wstring(port.begin(), port.end()) << "]: idx=" << std::to_wstring(index) << L", "
			   << L"Generic serial radio";

			return os.str();
		}

		auto OpenDevice() const -> RadioOperations * override
		{
			return loader();
		}

	private:
		const CreateRadioOps loader;
	};

	class SerialRadioFactory : public RadioOperationsFactory
	{
	public:
		auto ListDevices(const uint16_t &idx_offset) const -> const std::vector<RadioInfo *> override;

	private:
		auto OpDeviceList(std::function<void(const std::string &, const uint16_t &)>) const -> void;
	};
}