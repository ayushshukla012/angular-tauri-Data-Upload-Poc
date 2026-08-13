package com.insight.archtest;

import com.tngtech.archunit.core.importer.ClassFileImporter;
import com.tngtech.archunit.core.importer.ImportOption;
import com.tngtech.archunit.lang.ArchRule;
import org.junit.jupiter.api.Test;

import static com.tngtech.archunit.lang.syntax.ArchRuleDefinition.classes;

class LayeringRulesTest {

    private static final String[] SERVICE_PACKAGES = {
            "com.insight.upload",
            "com.insight.transformation",
            "com.insight.ocr",
            "com.insight.orchestrator",
            "com.insight.reporting"
    };

    @Test
    void controllersMustNotAccessRepositoriesDirectly() {
        var classes = new ClassFileImporter()
                .withImportOption(ImportOption.Predefined.DO_NOT_INCLUDE_TESTS)
                .importPackages(SERVICE_PACKAGES);

        ArchRule rule = classes()
                .that().resideInAPackage("..controller..")
                .should().onlyAccessClassesThat().resideOutsideOfPackage("..repository..");

        rule.check(classes);
    }

    @Test
    void entitiesMustNotBeAccessedFromControllers() {
        var classes = new ClassFileImporter()
                .withImportOption(ImportOption.Predefined.DO_NOT_INCLUDE_TESTS)
                .importPackages(SERVICE_PACKAGES);

        ArchRule rule = classes()
                .that().resideInAPackage("..entity..")
                .should().onlyBeAccessed().byClassesThat()
                .resideInAnyPackage("..entity..", "..repository..", "..service..", "..mapper..");

        rule.check(classes);
    }
}
