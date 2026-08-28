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
#include <radio_tool/device/ble_port.hpp>

#include <simpleble/SimpleBLE.h>

#include <algorithm>
#include <functional>
#include <map>
#include <chrono>
#include <stdexcept>
#include <thread>

using namespace radio_tool::device;

namespace
{
	//services known to carry a serial style link, in the order we prefer them.
	//Radios expose several services which look writable and notifiable, so
	//picking the first match discovers the wrong one
	const std::vector<std::string> SerialServices = {
		"0000ffe0-0000-1000-8000-00805f9b34fb", //the common HM-10 style module
		"0000fff0-0000-1000-8000-00805f9b34fb",
		"6e400001-b5a3-f393-e0a9-e50e24dcca9e", //Nordic UART
	};

	auto Lower(const std::string &s) -> std::string
	{
		std::string out = s;
		std::transform(out.begin(), out.end(), out.begin(),
					   [](unsigned char c) { return std::tolower(c); });
		return out;
	}

	/**
	 * The adapter to use, by name (hci0) or address, or the first one
	 */
	auto FindAdapter(const std::string &wanted) -> SimpleBLE::Adapter
	{
		if (!SimpleBLE::Adapter::bluetooth_enabled())
		{
			throw std::runtime_error("Bluetooth is not enabled on this machine");
		}

		auto adapters = SimpleBLE::Adapter::get_adapters();
		if (adapters.empty())
		{
			throw std::runtime_error("No Bluetooth adapter found");
		}

		if (wanted.empty())
		{
			return adapters[0];
		}

		for (auto &a : adapters)
		{
			if (Lower(a.identifier()) == Lower(wanted) || Lower(a.address()) == Lower(wanted))
			{
				return a;
			}
		}

		auto err = std::string("No Bluetooth adapter called '" + wanted + "', this machine has:");
		for (auto &a : adapters)
		{
			err += " " + a.identifier();
		}
		throw std::runtime_error(err);
	}
} // namespace

namespace
{
	/**
	 * Scan until fn accepts a device or the deadline passes, collecting every
	 * device seen along the way. A fixed length scan is unreliable here, the
	 * name of a radio often only arrives in a later advertisement.
	 */
	auto ScanUntil(SimpleBLE::Adapter &adapter, const uint32_t &scan_ms,
				   const std::function<bool(SimpleBLE::Peripheral &)> &fn)
		-> std::vector<SimpleBLE::Peripheral>
	{
		std::map<std::string, SimpleBLE::Peripheral> seen;

		adapter.scan_start();
		auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(scan_ms);
		auto done = false;
		while (!done && std::chrono::steady_clock::now() < deadline)
		{
			std::this_thread::sleep_for(std::chrono::milliseconds(250));
			for (auto &p : adapter.scan_get_results())
			{
				auto known = seen.find(p.address());
				//keep the entry which has a name, it may take a few
				//advertisements before the radio tells us what it is called
				if (known == seen.end() || (known->second.identifier().empty() && !p.identifier().empty()))
				{
					seen.insert_or_assign(p.address(), p);
				}
				if (fn && fn(p))
				{
					done = true;
				}
			}
		}
		adapter.scan_stop();

		std::vector<SimpleBLE::Peripheral> out;
		for (auto &kv : seen)
		{
			out.push_back(kv.second);
		}
		return out;
	}
} // namespace

auto BlePort::Adapters() -> std::vector<std::string>
{
	std::vector<std::string> out;
	for (auto &a : SimpleBLE::Adapter::get_adapters())
	{
		out.push_back(a.identifier() + " (" + a.address() + ")");
	}
	return out;
}

auto BlePort::Scan(const uint32_t &scan_ms, const std::string &adapter_name) -> std::vector<BleDevice>
{
	auto adapter = FindAdapter(adapter_name);

	std::vector<BleDevice> found;
	for (auto &p : ScanUntil(adapter, scan_ms, nullptr))
	{
		//devices with no name are of no use to anyone looking for their radio
		if (!p.identifier().empty())
		{
			found.push_back({p.address(), p.identifier()});
		}
	}
	return found;
}

BlePort::BlePort(const std::string &address, const std::string &adapter_name,
				 const uint32_t &scan_ms,
				 const std::string &write_uuid, const std::string &notify_uuid)
	: write_with_response(false)
{
	auto adapter = FindAdapter(adapter_name);
	auto wanted = Lower(address);

	for (auto &p : ScanUntil(adapter, scan_ms, [&wanted](SimpleBLE::Peripheral &found) {
			 return Lower(found.address()) == wanted;
		 }))
	{
		if (Lower(p.address()) == wanted)
		{
			peripheral = std::make_unique<SimpleBLE::Peripheral>(p);
			break;
		}
	}

	if (!peripheral)
	{
		throw std::runtime_error("No BLE device found at address " + address +
								 ", is the radio switched on with Bluetooth enabled?");
	}

	peripheral->connect();

	//find a service which behaves like a serial port, meaning it has a
	//characteristic we can write to and one which notifies us back. Both
	//must come from the same service
	auto examine = [&](SimpleBLE::Service &svc) {
		std::string notify_here, write_here;
		auto response_here = false, write_notifies = false;

		for (auto &chr : svc.characteristics())
		{
			auto uuid = Lower(chr.uuid());

			if (chr.can_notify() && notify_here.empty() &&
				(notify_uuid.empty() || uuid == Lower(notify_uuid)))
			{
				notify_here = chr.uuid();
			}

			auto writable = chr.can_write_command() || chr.can_write_request();
			if (writable && (write_uuid.empty() || uuid == Lower(write_uuid)))
			{
				//prefer the characteristic which also notifies: these modules
				//are usually one characteristic behaving like a serial port,
				//and radios answer on the one they were spoken to on
				if (write_here.empty() || (!write_notifies && chr.can_notify()))
				{
					write_here = chr.uuid();
					write_notifies = chr.can_notify();
					response_here = !chr.can_write_command();
				}
			}
		}

		if (notify_here.empty() || write_here.empty())
		{
			return false;
		}

		service = svc.uuid();
		notify_char = notify_here;
		write_char = write_here;
		write_with_response = response_here;
		return true;
	};

	auto services = peripheral->services();
	for (const auto &known : SerialServices)
	{
		for (auto &svc : services)
		{
			if (Lower(svc.uuid()) == known && examine(svc))
			{
				break;
			}
		}
		if (!service.empty())
		{
			break;
		}
	}

	//nothing recognisable, take the first service which could work
	for (auto &svc : services)
	{
		if (!service.empty())
		{
			break;
		}
		examine(svc);
	}

	if (notify_char.empty() || write_char.empty())
	{
		peripheral->disconnect();
		throw std::runtime_error("BLE device " + address + " has no serial service "
								 "(need one service with a characteristic to write on "
								 "and one to be notified on)");
	}

	peripheral->notify(service, notify_char,
					   [this](SimpleBLE::ByteArray payload) {
						   Receive(std::vector<uint8_t>(payload.data(), payload.data() + payload.size()));
					   });
}

BlePort::~BlePort()
{
	if (peripheral)
	{
		try
		{
			if (peripheral->is_connected())
			{
				peripheral->unsubscribe(service, notify_char);
				peripheral->disconnect();
			}
		}
		catch (...)
		{
			//nothing useful to do while tearing down
		}
	}
}

auto BlePort::Receive(const std::vector<uint8_t> &data) const -> void
{
	{
		std::lock_guard<std::mutex> lock(rx_lock);
		rx.insert(rx.end(), data.begin(), data.end());
	}
	rx_signal.notify_all();
}

auto BlePort::Write(const std::vector<uint8_t> &data) const -> void
{
	if (!peripheral || !peripheral->is_connected())
	{
		throw std::runtime_error("BLE connection to the radio was lost");
	}

	//a write cannot exceed the negotiated MTU, and the radio expects the
	//frame in order, so split anything larger and send it in sequence
	auto mtu = peripheral->mtu();
	auto chunk = (size_t)(mtu > 3 ? mtu - 3 : 20);

	for (size_t ix = 0; ix < data.size(); ix += chunk)
	{
		auto end = std::min(ix + chunk, data.size());
		SimpleBLE::ByteArray payload(std::vector<uint8_t>(data.begin() + ix, data.begin() + end));

		if (write_with_response)
		{
			peripheral->write_request(service, write_char, payload);
		}
		else
		{
			peripheral->write_command(service, write_char, payload);
		}
	}
}

auto BlePort::Read(const size_t &len, const uint32_t &timeout_ms) const -> std::vector<uint8_t>
{
	std::unique_lock<std::mutex> lock(rx_lock);
	rx_signal.wait_for(lock, std::chrono::milliseconds(timeout_ms),
					   [&] { return rx.size() >= len; });

	auto take = std::min(len, rx.size());
	std::vector<uint8_t> out(rx.begin(), rx.begin() + take);
	rx.erase(rx.begin(), rx.begin() + take);
	return out;
}

auto BlePort::FlushInput() const -> void
{
	std::lock_guard<std::mutex> lock(rx_lock);
	rx.clear();
}

auto BlePort::ToString() const -> std::string
{
	return "BLE " + (peripheral ? peripheral->address() : std::string("?")) +
		   " service " + service + ", write " + write_char +
		   (write_with_response ? " (with response)" : "") +
		   ", notify " + notify_char;
}
