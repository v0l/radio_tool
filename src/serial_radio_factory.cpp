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
#include <radio_tool/radio/serial_radio_factory.hpp>
#include <radio_tool/radio/radio.hpp>
#include <radio_tool/radio/ailunce_radio.hpp>
#include <radio_tool/radio/uv5r_radio.hpp>
#include <radio_tool/radio/uv17pro_radio.hpp>
#include <radio_tool/util.hpp>

#ifdef RADIO_TOOL_BLE
#include <radio_tool/device/ble_port.hpp>
#endif

#include <functional>
#include <memory>
#include <string>
#include <iostream>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN 1
#include <Windows.h>
#else
#include <filesystem>
namespace fs = std::filesystem;
#endif

using namespace radio_tool::radio;

struct DeviceMapper
{
	std::function<bool(const std::string&)> SupportsDevice;
	std::function<RadioOperations* (const std::string&)> CreateOperations;
};

const std::vector<DeviceMapper> Drivers = {
	{AilunceRadio::SupportsDevice, AilunceRadio::Create} };

auto SerialRadioFactory::SupportedModels() -> const std::vector<std::string>
{
	auto ret = std::vector<std::string>();
	for (const auto& m : UV5RRadio::Models())
	{
		ret.push_back(m.name);
	}
	for (const auto& m : UV17ProRadio::Models())
	{
		ret.push_back(m.name);
	}
	return ret;
}

auto SerialRadioFactory::OpenPort(const std::string& port, const std::string& model) -> RadioOperations*
{
	if (auto uv5r = UV5RRadio::GetModel(model))
	{
		return new UV5RRadio(port, *uv5r);
	}
	if (auto uv17 = UV17ProRadio::GetModel(model))
	{
		return new UV17ProRadio(port, *uv17);
	}

	auto err = std::string("Unknown radio model '" + model + "', supported models are:");
	for (const auto& m : SupportedModels())
	{
		err += " " + m;
	}
	throw std::runtime_error(err);
}

auto SerialRadioFactory::OpenBle(const std::string& address, const std::string& model, const std::string& adapter) -> RadioOperations*
{
#ifdef RADIO_TOOL_BLE
	auto uv17 = UV17ProRadio::GetModel(model);
	if (uv17 == nullptr)
	{
		auto err = std::string("Radio model '" + model + "' does not clone over Bluetooth, supported models are:");
		for (const auto& m : UV17ProRadio::Models())
		{
			err += std::string(" ") + m.name;
		}
		throw std::runtime_error(err);
	}

	auto ble = std::make_unique<device::BlePort>(address, adapter);
	return new UV17ProRadio(std::move(ble), *uv17);
#else
	(void)address;
	(void)model;
	(void)adapter;
	throw std::runtime_error("This build has no Bluetooth support, rebuild with -DBUILD_BLE=ON");
#endif
}

auto SerialRadioFactory::ListDevices(const uint16_t& idx_offset) const -> const std::vector<RadioInfo*>
{
	auto ret = std::vector<RadioInfo*>();

	OpDeviceList(
		[&ret, idx_offset](const std::string& port, const uint16_t& idx)
		{
			for (auto& driver : Drivers)
			{
				auto fnOpen = [&driver, port]()
				{
					return driver.CreateOperations(port);
				};

				if (driver.SupportsDevice(port))
				{
					ret.push_back(new SerialRadioInfo(fnOpen, port, idx_offset + idx));
				}
			}
		});
	return ret;
}

#ifdef _WIN32
auto SerialRadioFactory::OpDeviceList(std::function<void(const std::string&, const uint16_t&)> fn) const -> void
{
	HKEY comKey = nullptr;
	auto openResult = RegOpenKeyExA(HKEY_LOCAL_MACHINE, (LPSTR)"HARDWARE\\DEVICEMAP\\SERIALCOMM", 0, KEY_READ | KEY_WOW64_64KEY, &comKey);
	if (openResult != ERROR_SUCCESS)
	{
		// Registry key doesn't exist - no serial ports available
		return;
	}

	constexpr auto BufferSize = 1024L;
	auto idx = 0UL;
	char name[BufferSize] = {}, value[BufferSize] = {};
	while (1)
	{
		auto nameSize = BufferSize;
		auto valueSize = BufferSize;
		memset(name, 0, BufferSize);
		memset(value, 0, BufferSize);

		auto readResult = RegEnumValueA(comKey, idx, name, (LPDWORD)&nameSize, NULL, NULL, (LPBYTE)value, (LPDWORD)&valueSize);
		if (readResult == ERROR_SUCCESS)
		{
			fn(std::string(value), (uint16_t)idx);
		}
		else if (readResult == ERROR_NO_MORE_ITEMS)
		{
			break;
		}
		else
		{
			throw std::runtime_error("Error reading registory key values");
		}
		idx++;
	}

	RegCloseKey(comKey);
}

#else
auto SerialRadioFactory::OpDeviceList(std::function<void(const std::string&, const uint16_t&)> op) const -> void
{
#ifdef __APPLE__
	auto p = fs::path("/dev");
	auto idx = 0;
	for (const auto& de : fs::directory_iterator(p))
	{
		auto stem = de.path().stem().string();
		if (stem == "tty" || stem == "cu")
		{
			op(de.path().string(), idx++);
		}
	}
#else
	//https://stackoverflow.com/a/65764414
	auto p = fs::path("/dev/serial/by-id");
	if (!fs::exists(p))
	{
		return;
	}

	auto idx = 0;
	for (auto& de : fs::directory_iterator(p))
	{
		if (fs::is_symlink(de.symlink_status()))
		{
			auto canonical_path = fs::canonical(de);
			op(canonical_path.string(), idx++);
		}
	}
#endif
}
#endif