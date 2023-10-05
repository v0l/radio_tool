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
#include <radio_tool/hid/tyt_hid.hpp>
#include <radio_tool/util.hpp>

#include <stdexcept>

using namespace radio_tool::hid;

auto TYTHID::Setup() -> void
{
	auto err = 0;

	err = libusb_set_configuration(device, 0x01);
	if (err != LIBUSB_SUCCESS)
	{
		libusb_close(device);
		throw std::runtime_error(libusb_error_name(err));
	}
	err = libusb_claim_interface(device, 0x00);
	if (err != LIBUSB_SUCCESS)
	{
		libusb_close(device);
		throw std::runtime_error(libusb_error_name(err));
	}
	err = libusb_control_transfer(device, 0x21, 0x0a, 0, 0, nullptr, 0, timeout);
	if (err != LIBUSB_SUCCESS)
	{
		libusb_close(device);
		throw std::runtime_error(libusb_error_name(err));
	}
    /*
	auto buffer = (uint8_t*)malloc(64);
	auto tx = libusb_alloc_transfer(0);
	libusb_fill_interrupt_transfer(
		tx, device, TYTHID::EP_IN, buffer, 64, [](libusb_transfer* tx)
		{
			auto self = (TYTHID*)tx->user_data;
			self->OnTransfer(tx);
		},
		this, 5000);
	libusb_submit_transfer(tx);
    */
}

auto TYTHID::OnTransfer(libusb_transfer* tx) -> void
{
	if (tx->status == LIBUSB_TRANSFER_COMPLETED ||
		tx->status == LIBUSB_TRANSFER_TIMED_OUT)
	{
		{
			std::lock_guard<std::mutex> lk(signalCallback);
			this->tx = tx;
		}
		signalReady.notify_one();

		//wait again for item to be read
		{
			std::unique_lock<std::mutex> lk(signalCallback);
			auto tx_local = &this->tx;
			signalReady.wait(lk, [tx_local]
				{ return *tx_local == nullptr; });
		}
	}
	libusb_submit_transfer(tx);
}

auto TYTHID::SendCommand(const tyt::Command& cmd) -> tyt::Command
{
	std::vector<uint8_t> payload((int)cmd.data.size() + 4);
	std::fill(payload.begin(), payload.end(), 0x00);

	auto nums = (uint16_t*)payload.data();
	nums[0] = (uint16_t)cmd.type;
	nums[1] = cmd.data.size();
	std::copy(cmd.data.begin(), cmd.data.end(), payload.begin() + 4);

	InterruptWrite(TYTHID::EP_OUT, payload);
    auto data = InterruptRead(TYTHID::EP_IN, 42);
    auto type = ((uint16_t)data[1] << 8) | data[0];
    auto len  = ((uint16_t)data[3] << 8) | data[2];
    return tyt::Command((tyt::CommandType)type, len,
                            std::vector<uint8_t>(data.begin() + 4, data.begin() + 4 + len));
}

auto TYTHID::SendCommand(const std::vector<uint8_t>& cmd) -> tyt::Command
{
	return SendCommand(tyt::Command(tyt::CommandType::HostToDevice, cmd.size(), cmd));
}

auto TYTHID::SendCommand(const std::vector<uint8_t>& cmd, const uint8_t& size, const uint8_t& fill) -> tyt::Command
{
	auto ncmd = std::vector<uint8_t>(size, fill);
	std::copy(cmd.begin(), cmd.end(), ncmd.begin());

	return SendCommand(ncmd);
}

auto TYTHID::WaitForReply() -> tyt::Command
{
	std::unique_lock<std::mutex> lk(signalCallback);
	auto tx_local = &this->tx;
	signalReady.wait(lk, [tx_local]
		{ return *tx_local != nullptr; });

	if (tx->status == LIBUSB_TRANSFER_COMPLETED)
	{
		//setup return
		auto nums = (uint16_t*)tx->buffer;
		auto ret = tyt::Command((tyt::CommandType)nums[0], nums[1], std::vector<uint8_t>(tx->buffer + 4, tx->buffer + 4 + nums[1]));
		radio_tool::PrintHex(ret.data.begin(), ret.data.end());

		tx = nullptr;
		lk.unlock();
		signalReady.notify_one();
		return ret;
	}

	throw std::runtime_error("USB TRANSFER ERROR");
}

auto TYTHID::SendCommandAndOk(const tyt::Command& cmd) -> void
{
    auto ok = SendCommand(cmd);
	if (!(ok == tyt::OKResponse))
	{
		radio_tool::PrintHex(ok.data.begin(), ok.data.end());
		throw std::runtime_error("Invalid usb response from device");
	}
}

auto TYTHID::SendCommandAndOk(const std::vector<uint8_t>& cmd) -> void
{
    auto ok = SendCommand(cmd);
	if (!(ok == tyt::OKResponse))
	{
		radio_tool::PrintHex(ok.data.begin(), ok.data.end());
		throw std::runtime_error("Invalid usb response from device");
	}
}

auto TYTHID::SendCommandAndOk(const std::vector<uint8_t>& cmd, const uint8_t& size, const uint8_t& fill) -> void
{
    auto ok = SendCommand(cmd, size, fill);
	if (!(ok == tyt::OKResponse))
	{
		radio_tool::PrintHex(ok.data.begin(), ok.data.end());
		throw std::runtime_error("Invalid usb response from device");
	}
}