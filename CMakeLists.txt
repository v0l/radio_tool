cmake_minimum_required(VERSION 3.0.0)
project(tytfw VERSION 0.1.0)

include(CTest)
enable_testing()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(ALL_SRC 
    src/tytfw.cpp
    src/dfu.cpp
    src/tyt_dfu.cpp
)
add_executable(tytfw ${ALL_SRC})
target_include_directories(tytfw PUBLIC include)
target_link_libraries(tytfw usb-1.0)
target_compile_definitions(tytfw PRIVATE )

set(CPACK_PROJECT_NAME ${PROJECT_NAME})
set(CPACK_PROJECT_VERSION ${PROJECT_VERSION})
include(CPack)
