#include <radio_tool/device/tyt_device.hpp>

using namespace radio_tool::device;

auto TYTDevice::SetAddress(const uint32_t& addr) const -> void {
	this->_dfu.SetAddress(addr);
}

auto TYTDevice::Erase(const uint32_t& addr) const -> void {
	this->_dfu.Erase(addr);
}

auto TYTDevice::Write(const std::vector<uint8_t>& data) const -> void {
	this->_dfu.Download(data);
}

auto TYTDevice::Read(const uint16_t& size) const->std::vector<uint8_t> {

}