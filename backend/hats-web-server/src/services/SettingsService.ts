/**
 * SettingsService
 *
 * Service layer for managing persona and hat configuration settings.
 * Wraps SettingsRepository with domain-specific typed methods.
 *
 * Architecture:
 * - Repository handles raw key-value storage with JSON serialization
 * - Service provides typed, domain-specific access to settings
 * - Uses well-known keys for standard settings (persona.*, hat.*)
 *
 * Key Concepts:
 * - Persona: The personality/voice of the assistant (system prompt)
 * - Hat: The operational role/mode (e.g., planner, builder, validator)
 */

import { SettingsRepository } from "../repositories/SettingsRepository";

/**
 * Well-known setting keys used by the service
 */
export const SettingKeys = {
  /** Current active persona name */
  PERSONA_CURRENT: "persona.current",
  /** Map of persona name → definition */
  PERSONA_DEFINITIONS: "persona.definitions",
  /** Current active hat name */
  HAT_ACTIVE: "hat.active",
  /** Map of hat name → configuration */
  HAT_DEFINITIONS: "hat.definitions",
} as const;

/**
 * Default persona name when none is configured
 */
export const DEFAULT_PERSONA = "default";

/**
 * Default hat name when none is active
 */
export const DEFAULT_HAT = "hats";

/**
 * Fallback persona definition used when the default persona is not seeded in the database.
 * This ensures getCurrentPersonaDefinition() never returns undefined for the default persona.
 */
export const FALLBACK_PERSONA_DEFINITION: PersonaDefinition = {
  name: "Default",
  systemPrompt: "You are a helpful assistant.",
  description: "The default assistant persona",
};

/**
 * Fallback hat definition used when the default hat is not seeded in the database.
 * This ensures getActiveHatDefinition() never returns undefined for the default hat.
 */
export const FALLBACK_HAT_DEFINITION: HatDefinition = {
  name: "Hats",
  triggersOn: [
    "task.start",
    "build.task",
    "build.done",
    "build.blocked",
    "plan.start",
    "validation.done",
    "confession.clean",
    "confession.issues_found",
  ],
  publishes: [
    "plan.start",
    "build.task",
    "confession.issues_found",
    "confession.clean",
    "build.done",
    "validation.done",
  ],
  description: "Coordinates workflow, delegates to specialized hats",
};

/**
 * Definition of a persona (personality/voice)
 */
export interface PersonaDefinition {
  /** Display name for the persona */
  name: string;
  /** System prompt that defines the persona's behavior */
  systemPrompt: string;
  /** Optional description of the persona */
  description?: string;
}

/**
 * Configuration for a hat (operational role)
 */
export interface HatDefinition {
  /** Display name for the hat */
  name: string;
  /** Events this hat listens to */
  triggersOn: string[];
  /** Events this hat can publish */
  publishes: string[];
  /** Brief description of the hat's role */
  description: string;
  /** Additional instructions specific to this hat */
  instructions?: string;
}

/**
 * Map of persona names to their definitions
 */
export type PersonaMap = Record<string, PersonaDefinition>;

/**
 * Map of hat names to their configurations
 */
export type HatMap = Record<string, HatDefinition>;

/**
 * SettingsService
 *
 * Provides domain-specific access to persona and hat settings.
 */
export class SettingsService {
  private readonly repository: SettingsRepository;

  constructor(repository: SettingsRepository) {
    this.repository = repository;
  }

  // ============================================================
  // Persona Methods
  // ============================================================

  /**
   * Get the currently active persona name.
   * Returns DEFAULT_PERSONA if not configured.
   */
  getCurrentPersona(): string {
    return this.repository.get<string>(SettingKeys.PERSONA_CURRENT) ?? DEFAULT_PERSONA;
  }

  /**
   * Set the currently active persona.
   *
   * @param name - Persona name to activate
   */
  setCurrentPersona(name: string): void {
    this.repository.set(SettingKeys.PERSONA_CURRENT, name);
  }

  /**
   * Get all defined personas.
   * Returns empty map if none defined.
   */
  getPersonaDefinitions(): PersonaMap {
    return this.repository.get<PersonaMap>(SettingKeys.PERSONA_DEFINITIONS) ?? {};
  }

  /**
   * Get a specific persona definition by name.
   * Returns undefined if the persona doesn't exist.
   *
   * @param name - Persona name to retrieve
   */
  getPersona(name: string): PersonaDefinition | undefined {
    const definitions = this.getPersonaDefinitions();
    return definitions[name];
  }

  /**
   * Get the currently active persona's full definition.
   * Returns the fallback definition if the current persona is the default and not defined in the database.
   * Returns undefined only if a non-default persona is not defined.
   */
  getCurrentPersonaDefinition(): PersonaDefinition | undefined {
    const currentName = this.getCurrentPersona();
    const definition = this.getPersona(currentName);

    // Return fallback for default persona if not seeded in DB
    if (!definition && currentName === DEFAULT_PERSONA) {
      return FALLBACK_PERSONA_DEFINITION;
    }

    return definition;
  }

  /**
   * Set or update a persona definition.
   *
   * @param name - Persona name
   * @param definition - Persona configuration
   */
  setPersona(name: string, definition: PersonaDefinition): void {
    const definitions = this.getPersonaDefinitions();
    definitions[name] = definition;
    this.repository.set(SettingKeys.PERSONA_DEFINITIONS, definitions);
  }

  /**
   * Delete a persona definition.
   * If this is the current persona, the current persona is reset to default.
   *
   * @param name - Persona name to delete
   * @returns true if deleted, false if not found
   */
  deletePersona(name: string): boolean {
    const definitions = this.getPersonaDefinitions();
    if (!(name in definitions)) {
      return false;
    }

    delete definitions[name];
    this.repository.set(SettingKeys.PERSONA_DEFINITIONS, definitions);

    // Reset current persona if it was deleted
    if (this.getCurrentPersona() === name) {
      this.setCurrentPersona(DEFAULT_PERSONA);
    }

    return true;
  }

  /**
   * List all defined persona names.
   */
  listPersonas(): string[] {
    return Object.keys(this.getPersonaDefinitions());
  }

  /**
   * Check if a persona exists.
   *
   * @param name - Persona name to check
   */
  hasPersona(name: string): boolean {
    return name in this.getPersonaDefinitions();
  }

  // ============================================================
  // Hat Methods
  // ============================================================

  /**
   * Get the currently active hat name.
   * Returns DEFAULT_HAT if not configured.
   */
  getActiveHat(): string {
    return this.repository.get<string>(SettingKeys.HAT_ACTIVE) ?? DEFAULT_HAT;
  }

  /**
   * Set the currently active hat.
   *
   * @param name - Hat name to activate
   */
  setActiveHat(name: string): void {
    this.repository.set(SettingKeys.HAT_ACTIVE, name);
  }

  /**
   * Get all defined hats.
   * Returns empty map if none defined.
   */
  getHatDefinitions(): HatMap {
    return this.repository.get<HatMap>(SettingKeys.HAT_DEFINITIONS) ?? {};
  }

  /**
   * Get a specific hat definition by name.
   * Returns undefined if the hat doesn't exist.
   *
   * @param name - Hat name to retrieve
   */
  getHat(name: string): HatDefinition | undefined {
    const definitions = this.getHatDefinitions();
    return definitions[name];
  }

  /**
   * Get the currently active hat's full definition.
   * Returns the fallback definition if the active hat is the default and not defined in the database.
   * Returns undefined only if a non-default hat is not defined.
   */
  getActiveHatDefinition(): HatDefinition | undefined {
    const activeName = this.getActiveHat();
    const definition = this.getHat(activeName);

    // Return fallback for default hat if not seeded in DB
    if (!definition && activeName === DEFAULT_HAT) {
      return FALLBACK_HAT_DEFINITION;
    }

    return definition;
  }

  /**
   * Set or update a hat definition.
   *
   * @param name - Hat name
   * @param definition - Hat configuration
   */
  setHat(name: string, definition: HatDefinition): void {
    const definitions = this.getHatDefinitions();
    definitions[name] = definition;
    this.repository.set(SettingKeys.HAT_DEFINITIONS, definitions);
  }

  /**
   * Delete a hat definition.
   * If this is the active hat, the active hat is reset to default.
   *
   * @param name - Hat name to delete
   * @returns true if deleted, false if not found
   */
  deleteHat(name: string): boolean {
    const definitions = this.getHatDefinitions();
    if (!(name in definitions)) {
      return false;
    }

    delete definitions[name];
    this.repository.set(SettingKeys.HAT_DEFINITIONS, definitions);

    // Reset active hat if it was deleted
    if (this.getActiveHat() === name) {
      this.setActiveHat(DEFAULT_HAT);
    }

    return true;
  }

  /**
   * List all defined hat names.
   */
  listHats(): string[] {
    return Object.keys(this.getHatDefinitions());
  }

  /**
   * Check if a hat exists.
   *
   * @param name - Hat name to check
   */
  hasHat(name: string): boolean {
    return name in this.getHatDefinitions();
  }

  /**
   * Find hats that trigger on a specific event.
   *
   * @param eventName - Event to search for
   * @returns Array of hat names that listen to this event
   */
  findHatsByTrigger(eventName: string): string[] {
    const definitions = this.getHatDefinitions();
    return Object.entries(definitions)
      .filter(([_, hat]) => hat.triggersOn.includes(eventName))
      .map(([name, _]) => name);
  }

  // ============================================================
  // Generic Settings Access
  // ============================================================

  /**
   * Get a raw setting value (for non-standard settings).
   *
   * @param key - Setting key
   */
  getRaw<T>(key: string): T | undefined {
    return this.repository.get<T>(key);
  }

  /**
   * Set a raw setting value (for non-standard settings).
   *
   * @param key - Setting key
   * @param value - Value to store
   */
  setRaw<T>(key: string, value: T): void {
    this.repository.set(key, value);
  }

  /**
   * Delete a raw setting (for non-standard settings).
   *
   * @param key - Setting key
   */
  deleteRaw(key: string): boolean {
    return this.repository.delete(key);
  }
}
