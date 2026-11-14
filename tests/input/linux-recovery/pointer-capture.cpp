// The display query is a fixture boundary, just like the wrapped UHID device.
// Completion exercises production input.cpp without reading/moving a desktop.
#include <gtest/gtest.h>
#include "src/globals.h"
#include "src/input.h"
#include "src/platform/virtualhid_input.h"
#include <plank_transport_input.h>
#include <plank_transport_pointer.h>

namespace {
std::optional<util::point_t> queried_position;
}
namespace platf {
std::optional<util::point_t> get_mouse_loc(input_t&) { return queried_position; }
}

class PointerCompletionFixture : public ::testing::Test {
protected:
  void SetUp() override {
    ASSERT_FALSE(task_pool.running());
    auto platform = platf::input();
    auto& backend = platf::virtualhid::get_input_context(platform);
    backend = platf::virtualhid::input_context_t{lvh::BackendKind::fake};
    mouse = backend.mouse.get();
    ASSERT_NE(mouse, nullptr);
    input::testing::set_platform_input(std::move(platform));
    session = input::alloc(std::make_shared<safe::mail_raw_t>(),
      ::testing::UnitTest::GetInstance()->current_test_info()->name(), lease);
    input::testing::set_touch_port(session, {{0,0,1920,1080,1920,1080},1920,1080,0,0,1,1,1920,1080});
    queried_position = util::point_t{420.5,300.25};
  }
  void TearDown() override {
    input::reset(session,lease);
    static_cast<task_pool_util::TaskPool&>(task_pool) = task_pool_util::TaskPool{};
    session.reset(); input::testing::set_platform_input({}); queried_position.reset();
  }
  bool send(unsigned operation, unsigned sequence, std::vector<std::uint8_t>& reply) {
    PlankPointerPacket packet{static_cast<std::uint8_t>(operation),91,sequence,1920,1080,
      operation == PLANK_POINTER_BEGIN ? 120.f : 0.f,operation == PLANK_POINTER_BEGIN ? 240.f : 0.f};
    std::uint8_t bytes[PLANK_POINTER_PACKET_SIZE]; plank_pointer_encode(bytes,&packet);
    return input::native(session,PLANK_TRANSPORT_INPUT_POINTER_CAPTURE,bytes,sizeof(bytes),&reply);
  }
  bool absolute() {
    std::uint8_t bytes[PLANK_TRANSPORT_INPUT_ABSOLUTE_MOUSE_SIZE];
    plank_transport_input_encode_absolute_mouse(bytes,600,400,1920,1080);
    return input::native(session,PLANK_TRANSPORT_INPUT_ABSOLUTE_MOUSE,bytes,sizeof(bytes));
  }
  std::shared_ptr<input::input_t> session;
  std::uint64_t lease{};
  lvh::Mouse* mouse = nullptr;
};

TEST_F(PointerCompletionFixture, EndRequiresReleasedButtonsAndAcknowledgesQueriedPosition) {
  std::vector<std::uint8_t> reply;
  ASSERT_TRUE(send(PLANK_POINTER_BEGIN,1,reply)); EXPECT_TRUE(reply.empty());
  input::testing::handle_mouse_button(session,1,false);
  EXPECT_FALSE(send(PLANK_POINTER_END,2,reply)); EXPECT_TRUE(reply.empty());
  input::testing::handle_mouse_button(session,1,true);
  const auto submitted = mouse->submit_count();
  ASSERT_TRUE(send(PLANK_POINTER_END,2,reply));
  EXPECT_EQ(mouse->submit_count(),submitted); // Query/ACK never injects movement.
  PlankPointerPacket ack{};
  ASSERT_EQ(plank_pointer_decode(reply.data(),reply.size(),&ack),0);
  EXPECT_EQ(ack.operation,PLANK_POINTER_END_ACK); EXPECT_EQ(ack.epoch,91);
  EXPECT_EQ(ack.sequence,2); EXPECT_EQ(ack.width,1920); EXPECT_EQ(ack.height,1080);
  EXPECT_FLOAT_EQ(ack.x,420.5); EXPECT_FLOAT_EQ(ack.y,300.25);
  EXPECT_TRUE(absolute());
}

TEST_F(PointerCompletionFixture, FailedQueryCannotAcknowledgeOrResumeAbsoluteInput) {
  std::vector<std::uint8_t> reply;
  ASSERT_TRUE(send(PLANK_POINTER_BEGIN,1,reply));
  queried_position.reset();
  EXPECT_FALSE(send(PLANK_POINTER_END,2,reply)); EXPECT_TRUE(reply.empty());
  EXPECT_FALSE(absolute());
  queried_position = util::point_t{321,456};
  ASSERT_TRUE(send(PLANK_POINTER_END,2,reply));
  PlankPointerPacket ack{};
  ASSERT_EQ(plank_pointer_decode(reply.data(),reply.size(),&ack),0);
  EXPECT_FLOAT_EQ(ack.x,321); EXPECT_FLOAT_EQ(ack.y,456);
  EXPECT_TRUE(absolute());
}
