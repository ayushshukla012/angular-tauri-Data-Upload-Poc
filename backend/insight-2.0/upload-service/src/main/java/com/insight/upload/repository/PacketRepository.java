package com.insight.upload.repository;

import com.insight.upload.entity.Packet;
import org.springframework.data.jpa.repository.JpaRepository;

public interface PacketRepository extends JpaRepository<Packet, String> {
}
