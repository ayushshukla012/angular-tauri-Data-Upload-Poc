package com.insight.upload.repository;

import com.insight.upload.entity.Case;
import org.springframework.data.jpa.repository.JpaRepository;

public interface CaseRepository extends JpaRepository<Case, String> {
}
