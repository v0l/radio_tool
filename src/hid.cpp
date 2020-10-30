#include <radio_tool/hid/hid.hpp>

#include <stdexcept>

using namespace radio_tool::hid;

auto HID::InterruptRead(const uint8_t &ep, const uint16_t &len) const -> std::vector<uint8_t>
{
    std::vector<uint8_t> data(len);

    int rlen = 0;
    auto err = libusb_interrupt_transfer(device, ep, data.data(), len, &rlen, timeout);
    if (err != LIBUSB_SUCCESS)
    {
        throw std::runtime_error(libusb_error_name(err));
    }

    if (rlen != len)
    {
        data.reserve(rlen);
    }
    return data;
}

auto HID::InterruptWrite(const uint8_t &ep, const std::vector<uint8_t> &data) const -> void
{
    int rlen = 0;
    auto err = libusb_interrupt_transfer(device, ep, (unsigned char *)data.data(), data.size(), &rlen, timeout);
    if (err != LIBUSB_SUCCESS)
    {
        throw std::runtime_error(libusb_error_name(err));
    }

    if (rlen != data.size())
    {
        throw std::runtime_error("Invalid write len!");
    }
}

auto HID::BulkRead(const uint8_t &ep, const uint16_t &len) const -> std::vector<uint8_t>
{
    std::vector<uint8_t> data(len);

    int rlen = 0;
    auto err = libusb_interrupt_transfer(device, ep, data.data(), len, &rlen, timeout);
    if (err != LIBUSB_SUCCESS)
    {
        throw std::runtime_error(libusb_error_name(err));
    }

    if (rlen != len)
    {
        data.reserve(rlen);
    }
    return data;
}

auto HID::BulkWrite(const uint8_t &ep, const std::vector<uint8_t> &data) const -> void
{
    int rlen = 0;
    auto err = libusb_bulk_transfer(device, ep, (unsigned char *)data.data(), data.size(), &rlen, timeout);
    if (err != LIBUSB_SUCCESS)
    {
        throw std::runtime_error(libusb_error_name(err));
    }

    if (rlen != data.size())
    {
        throw std::runtime_error("Invalid write len!");
    }
}